//! The graph reservation lends the existing scalar peer-copy custody path.

use super::*;

pub(crate) struct PreparedGraphPeerCopyV1 {
    stream: RuntimeStreamIdV1,
    source: RuntimeMemoryRegionV1,
    destination: RuntimeMemoryRegionV1,
    prepared: peer_segments::PreparedPeerCopyV1,
}

impl<B: RuntimeBackendV1> RuntimeContextV1<B> {
    pub(crate) fn prepare_graph_peer_copy_v1(
        &self,
        stream: RuntimeStreamIdV1,
        source: RuntimeMemoryRegionV1,
        destination: RuntimeMemoryRegionV1,
    ) -> Result<PreparedContextGraphActionV1, RuntimeErrorV1<B::Error>> {
        let prepared = self.prepare_context_peer_copy_v1(stream, source, destination, &[], true)?;
        Ok(PreparedContextGraphActionV1::PeerCopy(
            PreparedGraphPeerCopyV1 {
                stream,
                source,
                destination,
                prepared,
            },
        ))
    }

    pub(super) fn submit_graph_peer_copy_v1(
        &mut self,
        token: ContextGraphReservationV1,
        copy: PreparedGraphPeerCopyV1,
    ) -> Result<RuntimeSubmissionV1<()>, RuntimeErrorV1<B::Error>> {
        self.require_graph_access(Some(token))?;
        self.require_stream_unheld_v1(copy.stream)?;
        let mut custody =
            self.prepare_scalar_peer_custody_v1(copy.stream, copy.source, copy.destination, &[])?;
        self.prepare_compute_peer_custody_v1(&mut custody)?;
        let prepared = copy.prepared;
        self.submit_context_operation_v1(
            copy.stream,
            prepared.stream_record,
            &[copy.destination.allocation],
            Some(PreparedSubmissionCustodyV1::Peer(
                PreparedPeerSubmissionV1 {
                    mechanism: PeerTransferMechanismV1::DeclaredPeerCopy {
                        contract_identity: peer_copy_contract_identity(
                            copy.stream,
                            copy.source,
                            copy.destination,
                        ),
                    },
                    scalar: Some(custody),
                },
            )),
            &[prepared.journal_source],
            |backend| {
                backend.peer_copy_v1(
                    prepared.stream_record.backend_stream,
                    prepared.source,
                    prepared.destination,
                    &prepared.dependencies,
                )
            },
        )
    }
}
