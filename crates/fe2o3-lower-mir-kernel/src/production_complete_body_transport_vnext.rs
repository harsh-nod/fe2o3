// Independent semantic transport census, not an authored-instruction interpreter.
// Raw rustc transport is independently checked by the live collector.
struct CompleteBodyArgumentTransportVNext {
    origin: [u8; COMPLETE_BODY_SOURCE_LOCAL_LIMIT],
    locals: usize,
    return_local: usize,
    arguments: [usize; 5],
    consumed: bool,
}
impl CompleteBodyArgumentTransportVNext {
    #[cfg(test)]
    fn new(locals: usize) -> Result<Self, ProductionSemanticKirErrorV1> {
        Self::with_roles(locals, 0, [1, 2, 3, 4, 5])
    }
    fn with_roles(
        locals: usize,
        return_local: usize,
        arguments: [usize; 5],
    ) -> Result<Self, ProductionSemanticKirErrorV1> {
        if !(6..=COMPLETE_BODY_SOURCE_LOCAL_LIMIT).contains(&locals) {
            return Err(complete_body_refusal("complete-body semantic local bound"));
        }
        if return_local >= locals
            || arguments.iter().enumerate().any(|(ordinal, local)| {
                *local >= locals || *local == return_local || arguments[..ordinal].contains(local)
            })
        {
            return Err(complete_body_refusal(
                "complete-body semantic role mapping differs",
            ));
        }
        let mut result = Self {
            origin: [u8::MAX; COMPLETE_BODY_SOURCE_LOCAL_LIMIT],
            locals,
            return_local,
            arguments,
            consumed: false,
        };
        for (ordinal, argument) in arguments.iter().copied().enumerate() {
            result.origin[argument] = ordinal as u8;
        }
        Ok(result)
    }
    fn temporary(&self, local: usize) -> Result<(), ProductionSemanticKirErrorV1> {
        if local >= self.locals || local == self.return_local || self.arguments.contains(&local) {
            Err(complete_body_refusal(
                "complete-body mutates argument/invalid temporary",
            ))
        } else {
            Ok(())
        }
    }
    fn storage(&mut self, local: usize) -> Result<(), ProductionSemanticKirErrorV1> {
        self.temporary(local)?;
        self.origin[local] = u8::MAX;
        Ok(())
    }
    fn assign(
        &mut self,
        to: usize,
        from: usize,
        moved: bool,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        self.temporary(to)?;
        let origin = self
            .origin
            .get(from)
            .copied()
            .filter(|_| from < self.locals)
            .filter(|origin| *origin != u8::MAX)
            .ok_or_else(|| complete_body_refusal("complete-body undefined argument transport"))?;
        if origin == 0 && !moved {
            return Err(complete_body_refusal("complete-body copies output owner"));
        }
        if moved {
            self.origin[from] = u8::MAX;
        }
        self.origin[to] = origin;
        Ok(())
    }
    fn consume_marker(
        &mut self,
        args: [u32; 5],
        moved: u8,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        if self.consumed || moved & 1 == 0 || moved & !31 != 0 {
            return Err(complete_body_refusal(
                "complete-body repeated marker/non-moved output",
            ));
        }
        for (ordinal, local) in args.into_iter().enumerate() {
            if self
                .origin
                .get(local as usize)
                .copied()
                .filter(|_| (local as usize) < self.locals)
                != Some(ordinal as u8)
            {
                return Err(complete_body_refusal(
                    "complete-body swapped/foreign argument transport",
                ));
            }
        }
        for (ordinal, local) in args.into_iter().enumerate() {
            if moved & (1 << ordinal) != 0 {
                self.origin[local as usize] = u8::MAX;
            }
        }
        self.consumed = true;
        Ok(())
    }
    fn require_marker(&self) -> Result<(), ProductionSemanticKirErrorV1> {
        if self.consumed {
            Ok(())
        } else {
            Err(complete_body_refusal("complete-body marker absent"))
        }
    }
}
