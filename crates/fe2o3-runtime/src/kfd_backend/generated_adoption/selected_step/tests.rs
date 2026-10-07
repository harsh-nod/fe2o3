//! Original-cell transition controls. Tokens below have no native authority.

use super::*;
use fe2o3_resource_accounting::{
    HostMetadataTableV1, ResourceCreditAccountV1, ResourceKindV1, ResourceVectorV1,
};
use std::{cell::Cell, rc::Rc};

struct Token {
    index: usize,
    drops: Rc<Cell<usize>>,
}
impl Drop for Token {
    fn drop(&mut self) {
        self.drops.set(self.drops.get() + 1);
    }
}

#[test]
fn arena1024_selected_step_retains_all_outstanding_original_cells_under_mixed_observations() {
    let account = ResourceCreditAccountV1::new(
        ResourceVectorV1::ZERO.with(ResourceKindV1::ControlResidentBytes, 1 << 20),
        1,
    )
    .unwrap();
    let drops = Rc::new(Cell::new(0));
    let mut receipts =
        HostMetadataTableV1::try_new(1024, Some(&account), || ReceiptV1::<Token, Token>::Ready)
            .unwrap();
    for (index, receipt) in receipts.iter_mut().enumerate() {
        step(
            &mut (),
            receipt,
            index,
            |_, index| {
                Ok(Token {
                    index,
                    drops: drops.clone(),
                })
            },
            |_, _| panic!("issue cannot poll"),
            |_, _| panic!("issue cannot recycle"),
        )
        .unwrap();
    }
    assert_eq!(drops.get(), 0);
    assert!(receipts.iter().enumerate().all(
        |(index, receipt)| matches!(receipt, ReceiptV1::Published(token) if token.index == index)
    ));
    let debit = account.usage();
    for (index, receipt) in receipts.iter_mut().enumerate() {
        step(
            &mut (),
            receipt,
            index,
            |_, _| panic!("must not republish"),
            |_, token| {
                assert_eq!(token.index, index);
                Ok(if index == 7 || index == 1023 {
                    SelectedPoll::Ready(token)
                } else {
                    SelectedPoll::Pending(token)
                })
            },
            |_, _| panic!("poll cannot recycle"),
        )
        .unwrap();
    }
    assert_eq!(drops.get(), 0);
    for index in [1023, 7] {
        step(
            &mut (),
            &mut receipts[index],
            index,
            |_, _| panic!(),
            |_, _| panic!(),
            |_, token| {
                assert_eq!(token.index, index);
                drop(token);
                Ok(())
            },
        )
        .unwrap();
        assert!(matches!(receipts[index], ReceiptV1::Recycled));
    }
    assert_eq!(drops.get(), 2);
    assert_eq!(
        account.usage(),
        debit,
        "selected terminal cells do not release the common roster debit"
    );
    for (index, receipt) in receipts.iter().enumerate() {
        if index != 7 && index != 1023 {
            assert!(matches!(receipt, ReceiptV1::Published(token) if token.index == index));
        }
    }
    // These are inert test tokens, not native owners or settlement evidence.
    drop(receipts);
    assert_eq!(drops.get(), 1024);
    assert_eq!(account.usage().used, ResourceVectorV1::ZERO);
}

#[test]
fn arena_selected_step_refusal_keeps_returned_original_and_unknown_never_progresses() {
    let drops = Rc::new(Cell::new(0));
    let mut receipt = ReceiptV1::<Token, Token>::Published(Token {
        index: 41,
        drops: drops.clone(),
    });
    assert!(
        step(
            &mut (),
            &mut receipt,
            41,
            |_, _| panic!(),
            |_, original| Err(SelectedPollFailure {
                error: Error::Contract("injected original return"),
                refused: Some(original),
            }),
            |_, _| panic!()
        )
        .is_err()
    );
    assert!(matches!(&receipt, ReceiptV1::Published(original) if original.index == 41));
    assert_eq!(drops.get(), 0);
    drop(receipt);
    let mut unknown = ReceiptV1::<Token, Token>::HandedToLower(receipt::HandoffV1::Poll);
    assert!(
        step(
            &mut (),
            &mut unknown,
            41,
            |_, _| panic!("unknown issue"),
            |_, _| panic!("unknown poll"),
            |_, _| panic!("unknown recycle")
        )
        .is_err()
    );
    assert!(matches!(
        unknown,
        ReceiptV1::HandedToLower(receipt::HandoffV1::Poll)
    ));
}

#[test]
fn arena_selected_step_refuses_both_singleton_rejection_states_without_consumption() {
    for mut receipt in [
        ReceiptV1::<Token, Token>::RejectedUnpublished {
            prior: receipt::RetirementV1::CancelledOnly,
            error: Error::Contract("original refused error"),
        },
        ReceiptV1::RejectedDisposed(Error::Contract("original disposed error")),
    ] {
        assert!(
            step(
                &mut (),
                &mut receipt,
                0,
                |_, _| panic!("singleton issue"),
                |_, _| panic!("singleton poll"),
                |_, _| panic!("singleton recycle")
            )
            .is_err()
        );
        match receipt {
            ReceiptV1::RejectedUnpublished { prior, error } => {
                assert_eq!(prior, receipt::RetirementV1::CancelledOnly);
                assert!(matches!(error, Error::Contract("original refused error")));
            }
            ReceiptV1::RejectedDisposed(error) => {
                assert!(matches!(error, Error::Contract("original disposed error")))
            }
            _ => panic!("singleton state was replaced"),
        }
    }
}
