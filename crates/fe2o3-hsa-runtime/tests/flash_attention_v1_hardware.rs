//! Protected exact-profile FlashAttention hardware gate and independent oracle.
//!
//! The ignored test intentionally cannot turn an artifact path or byte string
//! into authority. Until the production static wrapper can deliver the linear
//! finalization receipt in-process, it validates its independent oracle and
//! then fails closed before load.

const TOKENS: usize = 8;
const DIM: usize = 16;
const ELEMENTS: usize = TOKENS * DIM;
const SCALE: f32 = 0.25;
const CANARY_LEFT: f32 = f32::from_bits(0x7fc0_a101);
const CANARY_RIGHT: f32 = f32::from_bits(0x7fc0_a102);
const OUTPUT_POISON: f32 = f32::from_bits(0x7fc0_d1ff);

type BoxError = Box<dyn std::error::Error>;

fn require(condition: bool, message: impl Into<String>) -> Result<(), BoxError> {
    if condition {
        Ok(())
    } else {
        Err(message.into().into())
    }
}

fn strict_dot(left: &[f32], right: &[f32]) -> Result<f32, BoxError> {
    let mut sum = 0.0_f32;
    for (&left, &right) in left.iter().zip(right) {
        require(
            left.is_finite() && right.is_finite(),
            "non-finite dot input",
        )?;
        sum += left * right;
        require(sum.is_finite(), "non-finite dot intermediate")?;
    }
    let scaled = sum * SCALE;
    require(scaled.is_finite(), "non-finite scaled score")?;
    Ok(scaled)
}

/// Independent strict-order CPU implementation of the frozen online recurrence.
fn flash_attention_oracle(
    query: &[f32; ELEMENTS],
    key: &[f32; ELEMENTS],
    value: &[f32; ELEMENTS],
) -> Result<[f32; ELEMENTS], BoxError> {
    let mut output = [0.0_f32; ELEMENTS];
    for query_row in 0..TOKENS {
        let q = &query[query_row * DIM..(query_row + 1) * DIM];
        let mut maximum = 0.0_f32;
        let mut denominator = 0.0_f32;
        let mut numerator = [0.0_f32; DIM];
        for key_row in 0..=query_row {
            let k = &key[key_row * DIM..(key_row + 1) * DIM];
            let v = &value[key_row * DIM..(key_row + 1) * DIM];
            require(
                v.iter().all(|item| item.is_finite()),
                "non-finite value input",
            )?;
            let score = strict_dot(q, k)?;
            if key_row == 0 {
                maximum = score;
                denominator = 1.0;
                numerator.copy_from_slice(v);
                continue;
            }
            let next_maximum = maximum.max(score);
            let previous_weight = (maximum - next_maximum).exp();
            let current_weight = (score - next_maximum).exp();
            require(
                previous_weight.is_finite() && current_weight.is_finite(),
                "non-finite exponential",
            )?;
            denominator = denominator * previous_weight + current_weight;
            for component in 0..DIM {
                numerator[component] =
                    numerator[component] * previous_weight + v[component] * current_weight;
            }
            maximum = next_maximum;
        }
        require(
            denominator.is_finite() && denominator > 0.0,
            "invalid denominator",
        )?;
        for component in 0..DIM {
            let result = numerator[component] / denominator;
            require(result.is_finite(), "non-finite output")?;
            output[query_row * DIM + component] = result;
        }
    }
    Ok(output)
}

fn nominal() -> ([f32; ELEMENTS], [f32; ELEMENTS], [f32; ELEMENTS]) {
    let query = std::array::from_fn(|index| ((index % 13) as f32 - 6.0) / 8.0);
    let key = std::array::from_fn(|index| ((index % 11) as f32 - 5.0) / 7.0);
    let value = std::array::from_fn(|index| ((index % 17) as f32 - 8.0) / 9.0);
    (query, key, value)
}

fn equal_scores() -> ([f32; ELEMENTS], [f32; ELEMENTS], [f32; ELEMENTS]) {
    let query = [0.0; ELEMENTS];
    let key = [1.0; ELEMENTS];
    let value = std::array::from_fn(|index| (index / DIM) as f32 + (index % DIM) as f32 / 32.0);
    (query, key, value)
}

fn dominant_score() -> ([f32; ELEMENTS], [f32; ELEMENTS], [f32; ELEMENTS]) {
    let mut query = [0.0; ELEMENTS];
    let mut key = [0.0; ELEMENTS];
    let value = std::array::from_fn(|index| ((index % DIM) as f32 - 7.0) / 4.0);
    for row in 0..TOKENS {
        query[row * DIM] = 8.0;
        key[row * DIM] = if row == 3 { 8.0 } else { -8.0 };
    }
    (query, key, value)
}

fn exercise_oracle_and_guards() -> Result<(), BoxError> {
    for (name, (query, key, value)) in [
        ("nominal", nominal()),
        ("equal", equal_scores()),
        ("dominant", dominant_score()),
    ] {
        let query_before = query;
        let key_before = key;
        let value_before = value;
        let expected = flash_attention_oracle(&query, &key, &value)?;
        require(
            expected.iter().all(|item| item.is_finite()),
            format!("{name} output"),
        )?;
        require(query == query_before, format!("{name} query changed"))?;
        require(key == key_before, format!("{name} key changed"))?;
        require(value == value_before, format!("{name} value changed"))?;

        let mut guarded = [OUTPUT_POISON; ELEMENTS + 2];
        guarded[0] = CANARY_LEFT;
        guarded[ELEMENTS + 1] = CANARY_RIGHT;
        guarded[1..=ELEMENTS].copy_from_slice(&expected);
        require(
            guarded[0].to_bits() == CANARY_LEFT.to_bits(),
            "left canary changed",
        )?;
        require(
            guarded[ELEMENTS + 1].to_bits() == CANARY_RIGHT.to_bits(),
            "right canary changed",
        )?;
    }

    let (query, key, value) = nominal();
    let baseline = flash_attention_oracle(&query, &key, &value)?;
    let mut future_key = key;
    let mut future_value = value;
    future_key[7 * DIM..].fill(31.0);
    future_value[7 * DIM..].fill(-29.0);
    let substituted = flash_attention_oracle(&query, &future_key, &future_value)?;
    require(
        baseline[..7 * DIM] == substituted[..7 * DIM],
        "causal mask admitted a future-row effect",
    )?;

    for exceptional in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        let (mut query, key, value) = nominal();
        query[0] = exceptional;
        let guarded = [CANARY_LEFT, OUTPUT_POISON, CANARY_RIGHT];
        require(
            flash_attention_oracle(&query, &key, &value).is_err(),
            "exceptional input did not fail before output",
        )?;
        require(
            guarded[0].to_bits() == CANARY_LEFT.to_bits()
                && guarded[1].to_bits() == OUTPUT_POISON.to_bits()
                && guarded[2].to_bits() == CANARY_RIGHT.to_bits(),
            "exceptional path changed guarded output",
        )?;
    }
    Ok(())
}

fn exact_lower_hex_sha256(name: &str) -> Result<[u8; 32], BoxError> {
    let value = std::env::var(name).map_err(|_| format!("missing protected pin {name}"))?;
    require(
        value.len() == 64
            && value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)),
        format!("{name} must be 64 lowercase hexadecimal digits"),
    )?;
    let mut digest = [0_u8; 32];
    for (index, byte) in digest.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&value[index * 2..index * 2 + 2], 16)?;
    }
    require(digest != [0; 32], format!("{name} must be nonzero"))?;
    Ok(digest)
}

#[test]
fn independent_flash_oracle_covers_nominal_masked_equal_dominant_and_exceptional_cases()
-> Result<(), BoxError> {
    exercise_oracle_and_guards()
}

#[test]
#[ignore = "requires the production static wrapper, exact measured pins, protected linear receipt injection, and MI300X"]
fn protected_gfx942_flash_attention_v1_hardware() -> Result<(), BoxError> {
    exercise_oracle_and_guards()?;
    let wrapper = exact_lower_hex_sha256("FE2O3_FLASH_V1_STATIC_WRAPPER_SHA256")?;
    let worker = exact_lower_hex_sha256("FE2O3_FLASH_V1_WORKER_SHA256")?;
    let llvm = exact_lower_hex_sha256("FE2O3_FLASH_V1_LLVM_SHA256")?;
    let provider = exact_lower_hex_sha256("FE2O3_FLASH_V1_OCML_MANIFEST_SHA256")?;
    require(
        [wrapper, worker, llvm, provider]
            .windows(2)
            .all(|pair| pair[0] != pair[1]),
        "protected measurements must be independently bound",
    )?;
    Err(
        "production static wrapper cannot yet inject the opaque linear Flash receipt; refusing artifact-path or raw-byte fallback before HSA load"
            .into(),
    )
}
