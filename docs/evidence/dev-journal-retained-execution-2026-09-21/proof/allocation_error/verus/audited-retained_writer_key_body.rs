        $left.context_generation == $right.context_generation
            && $left.local == $right.local
            && match ($left.kind, $right.kind) {
                (WriterKindV1::Synchronous, WriterKindV1::Synchronous) => true,
                (WriterKindV1::Submission, WriterKindV1::Submission) => true,
                _ => false,
            }
