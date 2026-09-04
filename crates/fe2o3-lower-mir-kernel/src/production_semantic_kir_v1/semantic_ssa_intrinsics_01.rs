fn compiler_issued_ssa_bindings_v1(
    types: &[SemanticTypeDeclV1],
    callables: &[SemanticCallableDeclV1],
    function: &SemanticFunctionDeclV1,
    semantic_function: SemanticFunctionIdV1,
) -> Result<BTreeMap<SemanticTypeIdV1, SemanticPromotedBindingV1>, ProductionSemanticKirErrorV1> {
    let mut bindings = BTreeMap::new();
    for callable in callables {
        let SemanticCallableDeclV1::CompilerIntrinsic { operation, .. } = callable else {
            continue;
        };
        require_current_production_intrinsic_v1(operation)?;
        match operation {
            SemanticCompilerIntrinsicOperationV1::WorkgroupLdsScopeCurrent { scope } => {
                insert_compiler_issued_ssa_binding_v1(
                    &mut bindings,
                    *scope,
                    SemanticPromotedBindingV1::WorkgroupLdsScope,
                )?
            }
            SemanticCompilerIntrinsicOperationV1::MathContextCurrent { context } => {
                insert_compiler_issued_ssa_binding_v1(
                    &mut bindings,
                    *context,
                    SemanticPromotedBindingV1::MathContext,
                )?
            }
            SemanticCompilerIntrinsicOperationV1::CollectiveContextCurrent { context }
            | SemanticCompilerIntrinsicOperationV1::Gfx950SubgroupContextCurrent { context } => {
                insert_compiler_issued_ssa_binding_v1(
                    &mut bindings,
                    *context,
                    SemanticPromotedBindingV1::CollectiveContext,
                )?
            }
            SemanticCompilerIntrinsicOperationV1::MatrixContextCurrent { context } => {
                insert_compiler_issued_ssa_binding_v1(
                    &mut bindings,
                    *context,
                    SemanticPromotedBindingV1::MatrixContext,
                )?
            }
            SemanticCompilerIntrinsicOperationV1::WaveLaneCurrent { lane, wave_width } => {
                insert_compiler_issued_ssa_binding_v1(
                    &mut bindings,
                    *lane,
                    SemanticPromotedBindingV1::WaveLane {
                        wave_width: *wave_width,
                    },
                )?
            }
            SemanticCompilerIntrinsicOperationV1::DynamicLdsExactCurrent { scope, .. }
            | SemanticCompilerIntrinsicOperationV1::WorkgroupPipelineCreate { scope, .. } => {
                insert_compiler_issued_ssa_binding_v1(
                    &mut bindings,
                    *scope,
                    SemanticPromotedBindingV1::WorkgroupLdsScope,
                )?
            }
            SemanticCompilerIntrinsicOperationV1::Bf16MatrixLoadZeroFilledV2 {
                fragment,
                contract,
                storage_layout,
                ..
            }
            | SemanticCompilerIntrinsicOperationV1::Gfx950Fp4MatrixLoadM16K128 {
                fragment,
                contract,
                storage_layout,
                ..
            }
            | SemanticCompilerIntrinsicOperationV1::Gfx950Fp8MatrixLoadM16K128 {
                fragment,
                contract,
                storage_layout,
                ..
            } => insert_compiler_issued_ssa_binding_v1(
                &mut bindings,
                *fragment,
                SemanticPromotedBindingV1::MatrixFragment {
                    contract: *contract,
                    storage_layout: *storage_layout,
                },
            )?,
            SemanticCompilerIntrinsicOperationV1::Gfx950LdsTransposeCurrent {
                tile,
                format,
                ..
            } => insert_compiler_issued_ssa_binding_v1(
                &mut bindings,
                *tile,
                SemanticPromotedBindingV1::Gfx950LdsTransposeTile {
                    format: *format,
                    state: SemanticGfx950LdsTransposeStateV1::Uninitialized,
                },
            )?,
            SemanticCompilerIntrinsicOperationV1::Gfx950LdsTransposeStage {
                output_tile,
                format,
                ..
            } => insert_compiler_issued_ssa_binding_v1(
                &mut bindings,
                *output_tile,
                SemanticPromotedBindingV1::Gfx950LdsTransposeTile {
                    format: *format,
                    state: SemanticGfx950LdsTransposeStateV1::Staged,
                },
            )?,
            SemanticCompilerIntrinsicOperationV1::Gfx950LdsTransposePublish {
                output_tile,
                format,
                ..
            } => insert_compiler_issued_ssa_binding_v1(
                &mut bindings,
                *output_tile,
                SemanticPromotedBindingV1::Gfx950LdsTransposeTile {
                    format: *format,
                    state: SemanticGfx950LdsTransposeStateV1::Published,
                },
            )?,
            SemanticCompilerIntrinsicOperationV1::Gfx950LdsTransposeRead {
                fragment,
                contract,
                ..
            } => insert_compiler_issued_ssa_binding_v1(
                &mut bindings,
                *fragment,
                SemanticPromotedBindingV1::MatrixFragment {
                    contract: *contract,
                    storage_layout: SemanticMfmaStorageLayoutV1::RowMajor,
                },
            )?,
            SemanticCompilerIntrinsicOperationV1::F32MatrixAccumulatorZero {
                fragment,
                contract,
                ..
            }
            | SemanticCompilerIntrinsicOperationV1::MatrixMultiplyAccumulate {
                accumulator_fragment: fragment,
                accumulator: contract,
                ..
            } => insert_compiler_issued_ssa_binding_v1(
                &mut bindings,
                *fragment,
                SemanticPromotedBindingV1::AccumulatorFragment {
                    contract: *contract,
                },
            )?,
            SemanticCompilerIntrinsicOperationV1::WorkgroupReduceSum {
                scratch, element, ..
            } => insert_compiler_issued_ssa_binding_v1(
                &mut bindings,
                *scratch,
                SemanticPromotedBindingV1::WorkgroupCollectiveScratch { element: *element },
            )?,
            _ => {}
        }
    }

    for (block_index, block) in function.blocks().iter().enumerate() {
        let SemanticTerminatorKindV1::Call(call) = block.terminator().kind() else {
            continue;
        };
        let Some(SemanticCallableDeclV1::CompilerIntrinsic {
            operation:
                SemanticCompilerIntrinsicOperationV1::DynamicLdsExactCurrent {
                    dynamic_lds,
                    element_storage,
                    elements,
                    ..
                },
            ..
        }) = callables.get(call.callee().index() as usize)
        else {
            continue;
        };
        let destination = call.destination().ok_or_else(|| {
            unsupported(
                semantic_function.index(),
                Some(block_index as u32),
                None,
                "compiler-issued dynamic LDS producer has no destination",
            )
        })?;
        if !destination.place().projections().is_empty() || destination.place().ty() != *dynamic_lds
        {
            return Err(unsupported(
                semantic_function.index(),
                Some(block_index as u32),
                None,
                "compiler-issued dynamic LDS producer destination changed",
            ));
        }
        let storage = types.get(element_storage.index() as usize).ok_or_else(|| {
            unsupported(
                semantic_function.index(),
                Some(block_index as u32),
                None,
                "compiler-issued dynamic LDS storage type is missing",
            )
        })?;
        let element_size = storage.layout().size_bytes().ok_or_else(|| {
            unsupported(
                semantic_function.index(),
                Some(block_index as u32),
                None,
                "compiler-issued dynamic LDS storage is dynamically sized",
            )
        })?;
        let extent = u32::try_from(*elements).map_err(|_| {
            unsupported(
                semantic_function.index(),
                Some(block_index as u32),
                None,
                "compiler-issued dynamic LDS extent exceeds Kernel IR",
            )
        })?;
        let byte_extent = elements.checked_mul(element_size).ok_or_else(|| {
            unsupported(
                semantic_function.index(),
                Some(block_index as u32),
                None,
                "compiler-issued dynamic LDS byte extent overflows",
            )
        })?;
        let alignment = u32::try_from(storage.layout().alignment_bytes()).map_err(|_| {
            unsupported(
                semantic_function.index(),
                Some(block_index as u32),
                None,
                "compiler-issued dynamic LDS alignment exceeds Kernel IR",
            )
        })?;
        insert_compiler_issued_ssa_binding_v1(
            &mut bindings,
            *dynamic_lds,
            SemanticPromotedBindingV1::DynamicLds {
                dynamic_lds: *dynamic_lds,
                element_storage: *element_storage,
                elements: extent,
                byte_extent,
                alignment,
                producer_function: semantic_function,
                producer_block: SemanticBlockIdV1::from_index(block_index as u32),
            },
        )?;
    }

    let pipeline_contracts = workgroup_pipeline_type_contracts_v1(types, callables, &bindings)?;
    for callable in callables {
        let SemanticCallableDeclV1::CompilerIntrinsic {
            operation:
                SemanticCompilerIntrinsicOperationV1::WorkgroupPipelineCreate {
                    pipeline,
                    buffers,
                    elements,
                    prefetch_distance,
                    ..
                },
            ..
        } = callable
        else {
            continue;
        };
        let contract = pipeline_contracts.get(pipeline).ok_or_else(|| {
            unsupported(
                semantic_function.index(),
                None,
                None,
                "compiler-issued workgroup pipeline has no payload contract",
            )
        })?;
        let payload_binding = SemanticPipelinePayloadBindingV1::from_promoted(
            contract.payload_binding,
        )
        .ok_or_else(|| {
            unsupported(
                semantic_function.index(),
                None,
                None,
                "workgroup pipeline payload binding cannot cross SSA control flow",
            )
        })?;
        let packed_bits = pipeline_scalar_bit_width_v1(&contract.packed_type).ok_or_else(|| {
            unsupported(
                semantic_function.index(),
                None,
                None,
                "workgroup pipeline packed type has no exact bit width",
            )
        })?;
        insert_compiler_issued_ssa_binding_v1(
            &mut bindings,
            *pipeline,
            SemanticPromotedBindingV1::WorkgroupPipeline {
                pipeline: *pipeline,
                element: contract.element,
                payload_binding,
                buffers: *buffers,
                elements: *elements,
                prefetch_distance: *prefetch_distance,
                packed_bits,
                alignment: contract.alignment,
            },
        )?;
    }

    for callable in callables {
        let SemanticCallableDeclV1::CompilerIntrinsic { operation, .. } = callable else {
            continue;
        };
        match operation {
            SemanticCompilerIntrinsicOperationV1::MatrixMultiplyAccumulate {
                lhs_fragment,
                rhs_fragment,
                lhs,
                rhs,
                ..
            } => {
                for (fragment, expected) in [(*lhs_fragment, *lhs), (*rhs_fragment, *rhs)] {
                    if let Some(binding) = bindings.get(&fragment)
                        && !matches!(
                            binding,
                            SemanticPromotedBindingV1::MatrixFragment { contract, .. }
                                if *contract == expected
                        )
                    {
                        return Err(unsupported(
                            0,
                            None,
                            None,
                            "matrix consumer contract conflicts with its compiler-issued fragment type",
                        ));
                    }
                }
            }
            SemanticCompilerIntrinsicOperationV1::F32MatrixAccumulatorIntoValues {
                fragment,
                ..
            } => {
                if let Some(binding) = bindings.get(fragment)
                    && !matches!(
                        binding,
                        SemanticPromotedBindingV1::AccumulatorFragment { .. }
                    )
                {
                    return Err(unsupported(
                        0,
                        None,
                        None,
                        "accumulator projection conflicts with its compiler-issued fragment type",
                    ));
                }
            }
            SemanticCompilerIntrinsicOperationV1::Gfx950LdsTransposeStage {
                input_tile,
                format,
                ..
            } => {
                if let Some(binding) = bindings.get(input_tile)
                    && !matches!(
                        binding,
                        SemanticPromotedBindingV1::Gfx950LdsTransposeTile {
                            format: actual_format,
                            state: SemanticGfx950LdsTransposeStateV1::Uninitialized,
                        } if actual_format == format
                    )
                {
                    return Err(unsupported(
                        0,
                        None,
                        None,
                        "gfx950 LDS transpose stage input has conflicting compiler-issued state",
                    ));
                }
            }
            SemanticCompilerIntrinsicOperationV1::Gfx950LdsTransposePublish {
                input_tile,
                format,
                ..
            } => {
                if let Some(binding) = bindings.get(input_tile)
                    && !matches!(
                        binding,
                        SemanticPromotedBindingV1::Gfx950LdsTransposeTile {
                            format: actual_format,
                            state: SemanticGfx950LdsTransposeStateV1::Staged,
                        } if actual_format == format
                    )
                {
                    return Err(unsupported(
                        0,
                        None,
                        None,
                        "gfx950 LDS transpose publish input has conflicting compiler-issued state",
                    ));
                }
            }
            SemanticCompilerIntrinsicOperationV1::Gfx950LdsTransposeRead {
                tile, format, ..
            } => {
                if let Some(binding) = bindings.get(tile)
                    && !matches!(
                        binding,
                        SemanticPromotedBindingV1::Gfx950LdsTransposeTile {
                            format: actual_format,
                            state: SemanticGfx950LdsTransposeStateV1::Published,
                        } if actual_format == format
                    )
                {
                    return Err(unsupported(
                        0,
                        None,
                        None,
                        "gfx950 LDS transpose read input has conflicting compiler-issued state",
                    ));
                }
            }
            _ => {}
        }
    }
    Ok(bindings)
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SemanticWorkgroupPipelineTypeContractV1 {
    element: SemanticTypeIdV1,
    payload_binding: SemanticPromotedBindingV1,
    component_types: Box<[Type]>,
    packed_type: Type,
    alignment: u32,
}

fn pipeline_scalar_bit_width_v1(ty: &Type) -> Option<u32> {
    match ty.as_scalar()? {
        ScalarType::Index => Some(64),
        ScalarType::Bool => Some(8),
        scalar => scalar.bit_width().map(u32::from),
    }
}

fn pipeline_packed_type_v1(bits: u32) -> Option<Type> {
    Some(Type::Scalar(match bits {
        8 => ScalarType::U8,
        16 => ScalarType::U16,
        32 => ScalarType::U32,
        64 => ScalarType::U64,
        128 => ScalarType::U128,
        _ => return None,
    }))
}

fn pipeline_unsigned_component_type_v1(ty: &Type) -> Option<Type> {
    Some(Type::Scalar(match pipeline_scalar_bit_width_v1(ty)? {
        8 => ScalarType::U8,
        16 => ScalarType::U16,
        32 => ScalarType::U32,
        64 => ScalarType::U64,
        128 => ScalarType::U128,
        _ => return None,
    }))
}

fn pipeline_integer_constant_v1(ty: &Type, value: u64) -> Option<Constant> {
    match ty.as_scalar()? {
        ScalarType::U8 => u8::try_from(value).ok().map(Constant::U8),
        ScalarType::U16 => u16::try_from(value).ok().map(Constant::U16),
        ScalarType::U32 => u32::try_from(value).ok().map(Constant::U32),
        ScalarType::U64 => Some(Constant::U64(value)),
        _ => None,
    }
}

fn workgroup_pipeline_type_contracts_v1(
    types: &[SemanticTypeDeclV1],
    callables: &[SemanticCallableDeclV1],
    compiler_issued_bindings: &BTreeMap<SemanticTypeIdV1, SemanticPromotedBindingV1>,
) -> Result<
    BTreeMap<SemanticTypeIdV1, SemanticWorkgroupPipelineTypeContractV1>,
    ProductionSemanticKirErrorV1,
> {
    let mut elements = BTreeMap::new();
    for callable in callables {
        let SemanticCallableDeclV1::CompilerIntrinsic { operation, .. } = callable else {
            continue;
        };
        let (pipeline, element) = match operation {
            SemanticCompilerIntrinsicOperationV1::WorkgroupPipelineWrite { pipeline, element }
            | SemanticCompilerIntrinsicOperationV1::WorkgroupPipelineRead { pipeline, element } => {
                (*pipeline, *element)
            }
            _ => continue,
        };
        if let Some(existing) = elements.insert(pipeline, element)
            && existing != element
        {
            return Err(unsupported(
                0,
                None,
                None,
                "one workgroup pipeline type has inconsistent payload types",
            ));
        }
    }

    let mut contracts = BTreeMap::new();
    for (pipeline, element) in elements {
        let payload_binding = compiler_issued_bindings
            .get(&element)
            .copied()
            .unwrap_or(SemanticPromotedBindingV1::Ordinary);
        let component_types = payload_binding.transport_types(types, element)?;
        if component_types.is_empty() {
            return Err(unsupported(
                0,
                None,
                None,
                "workgroup pipeline payload has no physical components",
            ));
        }
        let component_bits = component_types.iter().try_fold(0_u32, |total, component| {
            let bits = pipeline_scalar_bit_width_v1(component).ok_or_else(|| {
                unsupported(
                    0,
                    None,
                    None,
                    "workgroup pipeline payload component is not a physical scalar",
                )
            })?;
            total.checked_add(bits).ok_or_else(|| {
                unsupported(0, None, None, "workgroup pipeline payload width overflows")
            })
        })?;
        let declaration = types.get(element.index() as usize).ok_or_else(|| {
            unsupported(0, None, None, "workgroup pipeline payload type is missing")
        })?;
        let layout_bits = declaration
            .layout()
            .size_bytes()
            .and_then(|bytes| bytes.checked_mul(8))
            .and_then(|bits| u32::try_from(bits).ok())
            .ok_or_else(|| {
                unsupported(
                    0,
                    None,
                    None,
                    "workgroup pipeline payload layout width is unavailable",
                )
            })?;
        if component_bits != layout_bits {
            return Err(unsupported(
                0,
                None,
                None,
                "workgroup pipeline payload transport does not cover its exact Rust layout",
            ));
        }
        let packed_type = pipeline_packed_type_v1(layout_bits).ok_or_else(|| {
            unsupported(
                0,
                None,
                None,
                "workgroup pipeline payload has no exact packed Kernel IR scalar",
            )
        })?;
        if layout_bits == 128 && component_types.as_slice() != [packed_type.clone()] {
            return Err(unsupported(
                0,
                None,
                None,
                "composite 128-bit workgroup pipeline packing is not executable",
            ));
        }
        let source_alignment = u32::try_from(declaration.layout().alignment_bytes())
            .ok()
            .filter(|alignment| *alignment != 0)
            .ok_or_else(|| {
                unsupported(
                    0,
                    None,
                    None,
                    "workgroup pipeline payload alignment is unavailable",
                )
            })?;
        let packed_alignment = layout_bits
            .checked_div(8)
            .filter(|alignment| *alignment != 0)
            .ok_or_else(|| {
                unsupported(
                    0,
                    None,
                    None,
                    "workgroup pipeline packed alignment is unavailable",
                )
            })?;
        let alignment = source_alignment.max(packed_alignment);
        contracts.insert(
            pipeline,
            SemanticWorkgroupPipelineTypeContractV1 {
                element,
                payload_binding,
                component_types: component_types.into_boxed_slice(),
                packed_type,
                alignment,
            },
        );
    }
    Ok(contracts)
}

fn require_current_production_intrinsic_v1(
    operation: &SemanticCompilerIntrinsicOperationV1,
) -> Result<(), ProductionSemanticKirErrorV1> {
    if matches!(
        operation,
        SemanticCompilerIntrinsicOperationV1::Bf16MatrixLoad { .. }
    ) {
        Err(unsupported(
            0,
            None,
            None,
            "the retired Option-returning BF16 matrix load is not admitted; use Bf16MatrixLoadZeroFilledV2",
        ))
    } else {
        Ok(())
    }
}
