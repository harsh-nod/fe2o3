// Independent semantic transport census, not an authored-instruction interpreter.
// Raw rustc transport is independently checked by the live collector.
struct PhysicalLdsExchangeArgumentTransportV22 {
    origin: [u8; PHYSICAL_LDS_EXCHANGE_SOURCE_LOCAL_LIMIT],
    locals: usize,
    return_local: usize,
    arguments: [usize; 2],
    consumed: bool,
}
impl PhysicalLdsExchangeArgumentTransportV22 {
    fn with_roles(
        locals: usize,
        return_local: usize,
        arguments: [usize; 2],
    ) -> Result<Self, ProductionSemanticKirErrorV1> {
        if !(3..=PHYSICAL_LDS_EXCHANGE_SOURCE_LOCAL_LIMIT).contains(&locals) {
            return Err(physical_lds_exchange_refusal(
                "physical-lds-exchange semantic local bound",
            ));
        }
        if return_local >= locals
            || arguments.iter().enumerate().any(|(ordinal, local)| {
                *local >= locals || *local == return_local || arguments[..ordinal].contains(local)
            })
        {
            return Err(physical_lds_exchange_refusal(
                "physical-lds-exchange semantic role mapping differs",
            ));
        }
        let mut result = Self {
            origin: [u8::MAX; PHYSICAL_LDS_EXCHANGE_SOURCE_LOCAL_LIMIT],
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
            Err(physical_lds_exchange_refusal(
                "physical-lds-exchange mutates argument/invalid temporary",
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
            .ok_or_else(|| {
                physical_lds_exchange_refusal("physical-lds-exchange undefined argument transport")
            })?;
        if origin == 1 && !moved {
            return Err(physical_lds_exchange_refusal(
                "physical-lds-exchange copies output owner",
            ));
        }
        if moved {
            self.origin[from] = u8::MAX;
        }
        self.origin[to] = origin;
        Ok(())
    }
    fn consume_marker(
        &mut self,
        args: [u32; 2],
        moved: u8,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        if self.consumed || moved & 2 == 0 || moved & !3 != 0 {
            return Err(physical_lds_exchange_refusal(
                "physical-lds-exchange repeated marker/non-moved output",
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
                return Err(physical_lds_exchange_refusal(
                    "physical-lds-exchange swapped/foreign argument transport",
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
            Err(physical_lds_exchange_refusal(
                "physical-lds-exchange marker absent",
            ))
        }
    }
}
