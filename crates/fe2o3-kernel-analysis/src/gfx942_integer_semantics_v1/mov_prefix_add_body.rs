// Loop annotations are proof-only tokens; both callers execute these same folds.
macro_rules! gfx942_mov_prefix_origins_body_v1 {
    ($syntax:ident, $moves:ident, $add:ident, $origins:ident, $index:ident,
     [$($initial_annotations:tt)*], [$($fold_annotations:tt)*]) => {
        $syntax!({
            let mut $origins = [Gfx942U32OriginV1::EntrySgpr(0); GFX942_ORDINARY_SGPR_COUNT_V1];
            let mut $index = 0usize;
            while $index < GFX942_ORDINARY_SGPR_COUNT_V1
                $($initial_annotations)*
            {
                $origins[$index] = Gfx942U32OriginV1::EntrySgpr($index as u8);
                $index += 1;
            }
            $index = 0;
            while $index < $moves.len()
                $($fold_annotations)*
            {
                let origin = match $moves[$index].source() {
                    Gfx942U32SourceV1::Sgpr(register) => $origins[register as usize],
                    Gfx942U32SourceV1::Constant(value) => Gfx942U32OriginV1::Constant(value),
                };
                $origins[$moves[$index].destination() as usize] = origin;
                $index += 1;
            }
            let sources = $add.sources();
            let left = match sources[0] {
                Gfx942U32SourceV1::Sgpr(register) => $origins[register as usize],
                Gfx942U32SourceV1::Constant(value) => Gfx942U32OriginV1::Constant(value),
            };
            let right = match sources[1] {
                Gfx942U32SourceV1::Sgpr(register) => $origins[register as usize],
                Gfx942U32SourceV1::Constant(value) => Gfx942U32OriginV1::Constant(value),
            };
            [left, right]
        })
    };
}

macro_rules! gfx942_mov_prefix_execute_body_v1 {
    ($syntax:ident, $moves:ident, $add:ident, $state:ident, $index:ident,
     [$($annotations:tt)*]) => {
        $syntax!({
            let mut $index = 0usize;
            while $index < $moves.len()
                $($annotations)*
            {
                $moves[$index].execute($state);
                $index += 1;
            }
            $add.execute($state)
        })
    };
}
