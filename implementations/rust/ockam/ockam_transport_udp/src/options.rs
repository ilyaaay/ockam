use crate::workers::Addresses;
use crate::UdpSizeOptions;
use ockam_core::compat::sync::Arc;
use ockam_core::flow_control::{FlowControlId, FlowControlOutgoingAccessControl, FlowControls};
use ockam_core::OutgoingAccessControl;

/// Options for a UDP connection
#[derive(Debug)]
pub struct UdpBindOptions {
    pub(super) consumer: Vec<FlowControlId>,
    pub(crate) flow_control_id: FlowControlId,
    pub(crate) size_options: UdpSizeOptions,
}

impl UdpBindOptions {
    #[allow(clippy::new_without_default)]
    /// Mark this Udp Receiver as a Producer with a random [`FlowControlId`]
    pub fn new() -> Self {
        Self {
            consumer: vec![],
            flow_control_id: FlowControls::generate_flow_control_id(),
            size_options: UdpSizeOptions::read_from_env(),
        }
    }

    /// Mark that this Connection is a Consumer for to the given [`FlowControlId`]
    pub fn as_consumer(mut self, id: &FlowControlId) -> Self {
        self.consumer.push(id.clone());

        self
    }

    /// Getter for freshly generated [`FlowControlId`]
    pub fn flow_control_id(&self) -> FlowControlId {
        self.flow_control_id.clone()
    }
}

impl Default for UdpBindOptions {
    fn default() -> Self {
        Self::new()
    }
}

impl UdpBindOptions {
    pub(crate) fn setup_flow_control(&self, flow_controls: &FlowControls, addresses: &Addresses) {
        flow_controls.add_producer(
            addresses.receiver_address(),
            &self.flow_control_id,
            None,
            vec![addresses.sender_address().clone()],
        );

        for id in &self.consumer {
            flow_controls.add_consumer(addresses.sender_address(), id);
        }
    }

    pub(crate) fn create_receiver_outgoing_access_control(
        &self,
        flow_controls: &FlowControls,
    ) -> Arc<dyn OutgoingAccessControl> {
        Arc::new(FlowControlOutgoingAccessControl::new(
            flow_controls,
            self.flow_control_id.clone(),
            None,
        ))
    }
}
