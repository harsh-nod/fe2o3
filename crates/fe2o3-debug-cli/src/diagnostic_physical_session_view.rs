//! Read-only projections implemented only for the three real typed sessions.
//! No trait object, owned transcript conversion, input constructor or resumption.
use super::profile::Profile;
use fe2o3_kernel_ir::{
    AddressSpace, CanonicalKernelIrVerificationResourceErrorV1 as Resource, ValueId,
};
use fe2o3_kir_debugger::{
    PhysicalEntryDebugNavigationV20 as Navigation, PhysicalEntryDebugSessionV20,
    PhysicalGlobalCopyDebugSessionV21, PhysicalLdsExchangeDebugSessionV22,
};
use fe2o3_kir_sim::{
    PhysicalEntryDebugBindingRefV20, PhysicalEntryDebugCaptureStopV20 as Stop,
    PhysicalEntryDebugOutcomeV20 as Outcome, PhysicalEntryDebugRecordRefV20,
    PhysicalGlobalCopyDebugBindingRefV21, PhysicalGlobalCopyDebugRecordRefV21,
    PhysicalLdsExchangeDebugBindingRefV22, PhysicalLdsExchangeDebugRecordRefV22, ScalarBitsV1,
    SimulationDebugCheckpointPhaseV1 as Phase, SimulationDebugSiteV1, SimulationInvocationV1,
};
pub(super) trait BindingView: Copy {
    fn value(self) -> ValueId;
    fn scalar(self) -> Option<ScalarBitsV1>;
    fn is_symbolic(self) -> bool;
    fn logical_pointer(self) -> Option<(u64, usize)>;
    fn logical_pointer_address_space(self) -> Option<AddressSpace>;
}
pub(super) trait RecordView: Copy {
    const PROFILE: Profile;
    type Binding: BindingView;
    fn invocation(self) -> SimulationInvocationV1;
    fn site(self) -> SimulationDebugSiteV1;
    fn phase(self) -> Option<Phase>;
    fn binding_count(self, depth: usize) -> Option<usize>;
    fn binding(self, depth: usize, index: usize) -> Option<Self::Binding>;
    fn memory_byte_at(self, index: usize, offset: usize) -> Option<(u8, bool)>;
    fn memory_allocation(self, index: usize) -> Option<(u64, usize)>;
    fn memory_address_space(self, index: usize) -> Option<AddressSpace>;
}
pub(super) trait SessionView {
    const PROFILE: Profile;
    type Record<'a>: RecordView
    where
        Self: 'a;
    fn records_len(&self) -> usize;
    fn cursor(&self) -> Option<usize>;
    fn record(&self, index: usize) -> Option<Self::Record<'_>>;
    fn current(&self) -> Option<Self::Record<'_>>;
    fn outcome(&self) -> Outcome;
    fn capture_stop(&self) -> Option<Stop>;
    fn charge_query_work(&mut self, work: usize) -> Result<(), Resource>;
    fn rewind(&mut self) -> Navigation;
    fn seek(&mut self, index: usize) -> Navigation;
}
// The sole implementations delegate without allocation or scalar reinterpretation.
macro_rules! views {
    ($session:ident, $record:ident, $binding:ident, $profile:ident) => {
        impl BindingView for $binding<'_> {
            fn value(self) -> ValueId {
                $binding::value(self)
            }
            fn scalar(self) -> Option<ScalarBitsV1> {
                $binding::scalar(self)
            }
            fn is_symbolic(self) -> bool {
                $binding::symbolic_kind(self).is_some()
            }
            fn logical_pointer(self) -> Option<(u64, usize)> {
                $binding::logical_pointer(self)
            }
            fn logical_pointer_address_space(self) -> Option<AddressSpace> {
                $binding::logical_pointer_address_space(self)
            }
        }
        impl<'a> RecordView for $record<'a> {
            const PROFILE: Profile = Profile::$profile;
            type Binding = $binding<'a>;
            fn invocation(self) -> SimulationInvocationV1 {
                $record::invocation(self)
            }
            fn site(self) -> SimulationDebugSiteV1 {
                $record::site(self)
            }
            fn phase(self) -> Option<Phase> {
                $record::phase(self)
            }
            fn binding_count(self, depth: usize) -> Option<usize> {
                $record::binding_count(self, depth)
            }
            fn binding(self, depth: usize, index: usize) -> Option<Self::Binding> {
                $record::binding(self, depth, index)
            }
            fn memory_byte_at(self, index: usize, offset: usize) -> Option<(u8, bool)> {
                $record::memory_byte_at(self, index, offset)
            }
            fn memory_allocation(self, index: usize) -> Option<(u64, usize)> {
                $record::memory_allocation(self, index)
            }
            fn memory_address_space(self, index: usize) -> Option<AddressSpace> {
                $record::memory_address_space(self, index)
            }
        }
        impl SessionView for $session {
            const PROFILE: Profile = Profile::$profile;
            type Record<'a> = $record<'a>;
            fn records_len(&self) -> usize {
                $session::records_len(self)
            }
            fn cursor(&self) -> Option<usize> {
                $session::cursor(self)
            }
            fn record(&self, index: usize) -> Option<Self::Record<'_>> {
                $session::record(self, index)
            }
            fn current(&self) -> Option<Self::Record<'_>> {
                $session::current(self)
            }
            fn outcome(&self) -> Outcome {
                $session::outcome(self)
            }
            fn capture_stop(&self) -> Option<Stop> {
                $session::capture_stop(self)
            }
            fn charge_query_work(&mut self, work: usize) -> Result<(), Resource> {
                $session::charge_query_work(self, work)
            }
            fn rewind(&mut self) -> Navigation {
                $session::rewind(self)
            }
            fn seek(&mut self, index: usize) -> Navigation {
                $session::seek(self, index)
            }
        }
    };
}
views!(
    PhysicalEntryDebugSessionV20,
    PhysicalEntryDebugRecordRefV20,
    PhysicalEntryDebugBindingRefV20,
    EntryV20
);
views!(
    PhysicalGlobalCopyDebugSessionV21,
    PhysicalGlobalCopyDebugRecordRefV21,
    PhysicalGlobalCopyDebugBindingRefV21,
    GlobalCopyV21
);

views!(
    PhysicalLdsExchangeDebugSessionV22,
    PhysicalLdsExchangeDebugRecordRefV22,
    PhysicalLdsExchangeDebugBindingRefV22,
    LdsExchangeV22
);
