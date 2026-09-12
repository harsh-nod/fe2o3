    #[test]
    fn launch_parser_preserves_nested_invisible_literal_groups() {
        let plain = parse_kernel_options(quote!(
            typed,
            launch(
                required = [4, 1, 1],
                max = [4, 1, 1],
                max_grid = [4294967295, 1, 1],
                static_shared_memory_bytes = 16
            )
        ))
        .unwrap();
        let mut dimension = quote!(4);
        let mut grid = quote!(4294967295);
        let mut bytes = quote!(16);
        for _ in 0..4 {
            dimension = invisible_type_group_v1(dimension);
            grid = invisible_type_group_v1(grid);
            bytes = invisible_type_group_v1(bytes);
            let array: syn::ExprArray = syn::parse2(quote!([#dimension, 1, 1])).unwrap();
            assert!(matches!(array.elems.first(), Some(Expr::Group(_))));
            let observed = parse_kernel_options(quote!(
                typed,
                launch(
                    required = [#dimension, 1, 1], max = [#dimension, 1, 1],
                    max_grid = [#grid, 1, 1], static_shared_memory_bytes = #bytes
                )
            ))
            .unwrap();
            assert_eq!(observed, plain);
            assert_eq!(
                super::encode_kernel_frontend_contract_v1(&observed).unwrap(),
                super::encode_kernel_frontend_contract_v1(&plain).unwrap(),
            );
        }
    }

    #[test]
    fn launch_parser_accepts_macro_forwarded_literal_tokens() {
        macro_rules! forwarded_launch {
            ($lanes:literal, $bytes:literal) => {
                quote!(
                    typed,
                    launch(
                        required = [$lanes, 1, 1],
                        max = [$lanes, 1, 1],
                        max_grid = [4, 1, 1],
                        static_shared_memory_bytes = $bytes
                    )
                )
            };
        }
        for (tokens, lanes, bytes) in [
            (forwarded_launch!(4, 16), 4, 16),
            (forwarded_launch!(8, 32), 8, 32),
        ] {
            let launch = parse_kernel_options(tokens).unwrap().launch.unwrap();
            assert_eq!(launch.required, Some([lanes, 1, 1]));
            assert_eq!(launch.maximum, Some([lanes, 1, 1]));
            assert_eq!(launch.max_grid, Some([4, 1, 1]));
            assert_eq!(launch.static_shared_memory_bytes, bytes);
        }
    }

    #[test]
    fn launch_invisible_groups_do_not_admit_nonliteral_or_invalid_dimensions() {
        for invalid in [
            quote!(2 + 2),
            quote!(LANES),
            quote!((4)),
            quote!(-1),
            quote!(4.0),
            quote!("4"),
            quote!(true),
            quote!(0),
            quote!(4294967296),
        ] {
            let grouped = invisible_type_group_v1(invisible_type_group_v1(invalid));
            for tokens in [
                quote!(launch(required = [#grouped, 1, 1])),
                quote!(launch(max = [#grouped, 1, 1])),
                quote!(launch(required = [4, 1, 1], max_grid = [#grouped, 1, 1])),
            ] {
                assert!(parse_kernel_options(tokens).is_err(), "accepted {grouped}");
            }
        }
        let limit = invisible_type_group_v1(quote!(1024));
        let too_large = invisible_type_group_v1(quote!(1025));
        assert!(parse_kernel_options(quote!(launch(required = [#limit, 1, 1]))).is_ok());
        for tokens in [
            quote!(launch(required = [#too_large, 1, 1])),
            quote!(launch(max = [#too_large, 1, 1])),
            quote!(launch(required = [#limit, 2, 1])),
        ] {
            assert!(parse_kernel_options(tokens).is_err());
        }
        let four = invisible_type_group_v1(quote!(4));
        assert!(
            parse_kernel_options(quote!(
                launch(required = [#four, 1, 1], max = [3, 1, 1])
            ))
            .is_err()
        );
    }
