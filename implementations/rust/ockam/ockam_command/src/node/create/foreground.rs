use crate::node::node_callback::NodeCallback;
use crate::node::CreateCommand;
use crate::util::foreground_args::wait_for_exit_signal;
use crate::CommandGlobalOpts;
use miette::miette;
use miette::IntoDiagnostic;
use ockam::tcp::{TcpListenerOptions, TcpTransport};
use ockam::udp::{UdpBindArguments, UdpBindOptions, UdpTransport};
use ockam::Address;
use ockam::Context;
use ockam_api::fmt_log;
use ockam_api::nodes::service::{NodeManagerTransport, SecureChannelType};
use ockam_api::nodes::InMemoryNode;
use ockam_api::nodes::{
    service::{NodeManagerGeneralOptions, NodeManagerTransportOptions},
    NodeManagerWorker, NODEMANAGER_ADDR,
};
use ockam_api::terminal::notification::NotificationHandler;

use ockam_core::LOCAL;
use std::sync::Arc;
use tokio::time::{sleep, Duration};
use tracing::{debug, info, instrument};

impl CreateCommand {
    #[instrument(skip_all, fields(node_name = self.name))]
    pub(super) async fn foreground_mode(
        &self,
        ctx: &Context,
        opts: CommandGlobalOpts,
    ) -> miette::Result<()> {
        let node_name = self.name.clone();
        debug!("creating node in foreground mode");

        let trust_options = opts
            .state
            .retrieve_trust_options(
                &self.trust_opts.project_name,
                &self.trust_opts.authority_identity,
                &self.trust_opts.authority_route,
                &self.trust_opts.credential_scope,
            )
            .await
            .into_diagnostic()?;

        // Create TCP transport
        let tcp = TcpTransport::create(ctx).into_diagnostic()?;
        let tcp_listener = tcp
            .listen(&self.tcp_listener_address, TcpListenerOptions::new())
            .await
            .into_diagnostic()?;
        info!("TCP listener at {}", tcp_listener.socket_address());

        // Set node_name so that node can isolate its data in the storage from other nodes
        self.get_or_create_identity(&opts, &self.identity).await?;
        let _notification_handler = if self.foreground_args.child_process {
            // If enabled, the user's terminal would receive notifications
            // from the node after the command exited.
            None
        } else {
            // Enable the notifications only on explicit foreground nodes.
            Some(NotificationHandler::start(
                &opts.state,
                opts.terminal.clone(),
            ))
        };
        let node_info = opts
            .state
            .start_node_with_optional_values(&node_name, &self.identity, Some(&tcp_listener))
            .await?;
        debug!("node info persisted {node_info:?}");

        let udp_options = if self.udp {
            let udp = UdpTransport::create(ctx).into_diagnostic()?;
            let options = UdpBindOptions::new();
            let flow_control_id = options.flow_control_id();
            udp.bind(
                UdpBindArguments::new().with_bind_address(&self.udp_listener_address)?,
                options,
            )
            .await?;

            Some(NodeManagerTransport::new(flow_control_id, udp))
        } else {
            None
        };

        let in_memory_node = InMemoryNode::new(
            ctx,
            NodeManagerGeneralOptions::new(
                opts.state.clone(),
                node_name.clone(),
                self.launch_configuration.is_none(),
                self.status_endpoint_port(),
                true,
            ),
            NodeManagerTransportOptions::new(
                NodeManagerTransport::new(tcp_listener.flow_control_id().clone(), tcp),
                udp_options,
            ),
            trust_options,
        )
        .await
        .into_diagnostic()?;
        debug!("in-memory node created");

        let in_memory_node = Arc::new(in_memory_node);
        let node_manager_worker = NodeManagerWorker::new(in_memory_node.clone());
        ctx.flow_controls()
            .add_consumer(&NODEMANAGER_ADDR.into(), tcp_listener.flow_control_id());
        ctx.start_worker(NODEMANAGER_ADDR, node_manager_worker)
            .into_diagnostic()?;
        debug!("node manager worker started");

        if self
            .start_secure_channel_listener(ctx, &in_memory_node, &opts)
            .await
            .is_err()
        {
            //TODO: Process should terminate on any error during its setup phase,
            //      not just during the start_services.
            //TODO: This sleep here is a workaround on some orchestrated environment,
            //      the lmdb db, that is used for policy storage, fails to be re-opened
            //      if it's still opened from another docker container, where they share
            //      the same pid. By sleeping for a while we let this container be promoted
            //      and the other being terminated, so when restarted it works.  This is
            //      FAR from ideal.
            sleep(Duration::from_secs(10)).await;
            ctx.shutdown_node().await.into_diagnostic()?;
            return Err(miette!("Failed to start services"));
        }

        let node_resources = in_memory_node.get_node_resources().await?;
        opts.terminal
            .clone()
            .stdout()
            .plain(self.plain_output(&opts, &node_name).await?)
            .machine(&node_name)
            .json_obj(&node_resources)?
            .write_line()?;

        if let Some(tcp_callback_port) = self.tcp_callback_port {
            NodeCallback::signal(tcp_callback_port);
        }

        wait_for_exit_signal(
            &self.foreground_args,
            &opts,
            "To exit and stop the Node, please press Ctrl+C\n",
        )
        .await?;

        // Clean up and exit
        let _ = opts.state.stop_node(&node_name).await;

        Ok(())
    }

    async fn start_secure_channel_listener(
        &self,
        ctx: &Context,
        in_memory_node: &InMemoryNode,
        opts: &CommandGlobalOpts,
    ) -> miette::Result<()> {
        let launch_configuration = if let Some(launch_configuration) = &self.launch_configuration {
            launch_configuration
        } else {
            return Ok(());
        };

        let startup_services =
            if let Some(startup_services) = &launch_configuration.startup_services {
                startup_services
            } else {
                return Ok(());
            };

        if let Some(cfg) = startup_services.secure_channel_listener.as_ref() {
            if cfg.disabled {
                return Ok(());
            }

            opts.terminal
                .write_line(fmt_log!("Starting secure-channel listener ..."))?;
            in_memory_node
                .create_secure_channel_listener(
                    Address::from((LOCAL, cfg.address.clone())),
                    cfg.authorized_identifiers.clone(),
                    cfg.identity.clone(),
                    ctx,
                    SecureChannelType::KeyExchangeAndMessages,
                )
                .await?;
        }

        Ok(())
    }
}
