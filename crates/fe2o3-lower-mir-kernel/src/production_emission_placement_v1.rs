// Source block/value identities remain local; only emitted identities are placed.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct SemanticEmissionPlacementV1 {
    first_block: u32,
    first_value: u32,
}

impl SemanticEmissionPlacementV1 {
    fn block(self, source: u32) -> Result<BlockId, ProductionSemanticKirErrorV1> {
        self.first_block
            .checked_add(source)
            .map(BlockId)
            .ok_or_else(|| unsupported(0, Some(source), None, "expanded block identity overflow"))
    }

    fn value_floor(self, parameters: &[ValueId]) -> Result<u32, ProductionSemanticKirErrorV1> {
        parameters
            .iter()
            .try_fold(self.first_value, |floor, value| {
                value
                    .0
                    .checked_add(1)
                    .map(|next| floor.max(next))
                    .ok_or_else(|| {
                        unsupported(0, None, None, "parameter value identity has no successor")
                    })
            })
    }
}

impl SemanticFunctionLoweringV1<'_> {
    fn kernel_block_id_v1(
        &self,
        source: SemanticBlockIdV1,
    ) -> Result<BlockId, ProductionSemanticKirErrorV1> {
        self.emission_placement.block(source.index())
    }
}

#[cfg(test)]
mod emission_placement_tests {
    use super::*;

    #[test]
    fn emission_placement_preserves_local_order_without_value_collisions() {
        let placement = SemanticEmissionPlacementV1 {
            first_block: 17,
            first_value: 100,
        };
        assert_eq!(placement.block(0).unwrap(), BlockId(17));
        assert_eq!(placement.block(9).unwrap(), BlockId(26));
        assert_eq!(placement.value_floor(&[]).unwrap(), 100);
        assert_eq!(
            placement.value_floor(&[ValueId(3), ValueId(99)]).unwrap(),
            100
        );
        assert_eq!(
            placement.value_floor(&[ValueId(105), ValueId(3)]).unwrap(),
            106
        );
    }

    #[test]
    fn emission_placement_rejects_exhausted_identity_space() {
        let placement = SemanticEmissionPlacementV1 {
            first_block: u32::MAX,
            first_value: 0,
        };
        assert_eq!(placement.block(0).unwrap(), BlockId(u32::MAX));
        assert!(placement.block(1).is_err());
        assert!(placement.value_floor(&[ValueId(u32::MAX)]).is_err());
        assert!(
            placement
                .value_floor(&[ValueId(u32::MAX), ValueId(0)])
                .is_err()
        );
    }
}
