#![cfg_attr(not(feature = "std"), no_std, no_main)]

/**
 * This is a simple ink! contract that tracks user contributions towards a wishlist.
 * Users can create a wishlist of how much tokens they want to contribute.
 *
 * All wishlists created a publicly visible by anyone
 * Others can contribute towards the wishlist also
 * A wishlist can have a description, id, end_date, target, contributors, raised
 *
 * After the end_date, if the raised is gt target, the owner takes the profit
 * After the end date if the contributions are less than target, contributors share the raised amount
 *
 * @Storage
 * - next_item_id: u32
 * - items_by_id: Mapping<Account, Vec<WishListItem>>
 * - items: StorageVec<WishlistItem>
 *
 * @Messages
 * - add_wishlist_item(description: String, goal: Balance, end_date: Timestamp);
 * - claim_wish(id: u32);
 * - split_rewards(id: u32);
 * - get_wishlist_item(id: u32);
 * - get_user_wishes(account: AccountId);
 *
 * - fund_wish(id: u32, owner: AccountId);
 */

#[ink::contract]
mod wishlist {
    use ink::{
        storage::{StorageVec},
        H160, U256,
    };

    use ink::prelude::{string::String, vec::Vec};

    #[ink(event)]
    pub struct WishlistAdded {
        #[ink(topic)]
        id: u32,
        owner: H160,
    }

    /// Errors that can occur upon calling this contract.
    #[derive(Debug, PartialEq, Eq)]
    #[ink::scale_derive(Encode, Decode, TypeInfo)]
    pub enum Error {
        /// Returned if the name already exists upon registration.
        InvalidContribution,
        /// Returned if wish does not exist.
        WishNotFound,
        /// Invalid Target amount
        InvalidTarget,
    }

    /// Type alias for the contract's result type.
    pub type Result<T> = core::result::Result<T, Error>;

    #[cfg_attr(
        feature = "std",
        derive(Debug, PartialEq, Eq, ink::storage::traits::StorageLayout)
    )]
    #[ink::scale_derive(Encode, Decode, TypeInfo)]
    pub struct WishListItem {
        id: u32,
        description: String,
        owner: H160,
        target: U256,
        end_date: u64,
        raised: U256,
        contributors: Vec<(H160, U256)>,
    }

    /// Defines the storage of your contract.
    /// Add new fields to the below struct in order
    /// to add new static storage fields to your contract.
    #[ink(storage)]
    pub struct Wishlist {
        /// Stores the id of the next wishlist item.
        /// The current length will be (next_item_id - 1)
        next_item_id: u32,
        // items_by_id: Mapping<H160, Vec<WishListItem>>,
        items: StorageVec<Option<WishListItem>>,
    }

    impl Wishlist {
        /// Constructor that initializes the contract.
        #[ink(constructor)]
        pub fn new() -> Self {
            Self {
                next_item_id: 1,
                items: StorageVec::new(),
            }
        }

        /// Constructor that initializes the `bool` value to `false`.
        ///
        /// Constructors can delegate to other constructors.
        #[ink(constructor)]
        pub fn default() -> Self {
            Self::new()
        }

        /// add a wishlist item to the accountId
        #[ink(message, payable)]
        pub fn add_wishlist_item(
            &mut self,
            description: String,
            end_date: u64,
            target: U256,
        ) -> Result<()> {
            let caller = self.env().caller();
            let value = self.env().transferred_value();

            // Ensure target is not 0
            if target <= U256::zero() {
                return Err(Error::InvalidTarget);
            }

            // The trasferred_value > 10% of target
            let ten_percent = (target * U256::from(10)) / U256::from(100);
            if value < ten_percent {
                return Err(Error::InvalidContribution);
            }
            let item_count = self.next_item_id;

            let wishlist = WishListItem {
                id: item_count,
                description,
                owner: caller,
                target,
                end_date,
                raised: value,
                contributors: Vec::new(),
            };

            self.next_item_id = self
                .next_item_id
                .checked_add(1)
                .ok_or(Error::InvalidContribution)?;
            self.items.push(&Some(wishlist));
            self.env().emit_event(WishlistAdded {
                id: item_count,
                owner: caller,
            });

            Ok(())
        }

        #[ink(message, payable)]
        pub fn fund_wish(&mut self, id: u32) -> Result<()> {
            let caller = self.get_caller();
            let value = self.env().transferred_value();
            if value <= U256::zero() {
                return Err(Error::InvalidContribution);
            }

            let wishlist = self.items.get(id);
            match wishlist {
                None => Err(Error::WishNotFound),
                Some(item) => {
                    let mut item = item.unwrap();
                    if caller == item.owner {
                        // If owner is funding, update the raised amount
                        item.raised = item.raised + value;
                        self.items.set(id, &Some(item));
                    } else {
                        // If contributor exists, update contribution
                        let contributor_exists = item.contributors.iter().any(|c| c.0 == caller);
                        if contributor_exists {
                            let contributors: Vec<(H160, U256)> = item
                                .contributors
                                .iter_mut()
                                .map(|c| {
                                    if c.0 == caller {
                                        c.1 = c.1 + value;
                                    }
                                    return *c;
                                })
                                .collect::<Vec<(H160, U256)>>();

                            item.contributors = contributors.clone();
                        } else {
                            // If contributor does not exist, add to contributors
                            item.contributors.push((caller, value));
                        }

                        self.items.set(id, &Some(item));
                    }
                    Ok(())
                }
            }
        }

        #[ink(message)]
        pub fn split_raised_wish(&mut self, id: u32) -> Result<()> {
            let caller = self.get_caller();
            let wishlist = self.items.get(id);

            match wishlist {
                None => Err(Error::WishNotFound),
                Some(item) => {
                    let item = item.unwrap();
                    // owner must be a contributor
                    assert!(
                        item.contributors.iter().find(|c| c.0 == caller).is_some(),
                        "Caller is not a contributor"
                    );
                    let contributors_raise = self.get_contributors_raised(id);
                    let total_worth = match contributors_raise {
                        None => item.raised,
                        Some(raised) => raised + item.raised,
                    };

                    let contributors = item.contributors;
                    self.items.set(
                        id,
                        &None::<WishListItem>,
                    );

                    for (address, bal) in contributors {
                        let percentage = (bal * U256::from(100)) / total_worth;
                        let _ = self.env().transfer(address, percentage);
                    }

                    Ok(())
                }
            }
        }

        #[ink(message)]
        pub fn claim_wish(&mut self, id: u32) -> Result<()> {
            // Ensure the caller is the caller
            let caller = self.get_caller();

            let wishlist = self.items.get(id);
            match wishlist {
                None => Err(Error::WishNotFound),
                Some(item) => {
                    let item = item.unwrap();
                    if item.owner != caller {
                        return Err(Error::WishNotFound);
                    } else {
                        let time = self.env().block_timestamp();
                        assert!(time >= item.end_date, "Cannot claim wish before end date");
                        assert!(item.owner == caller, "Only owner can claim wish");

                        if item.raised >= item.target {
                            let contributors_worth: U256 = item
                                .contributors
                                .iter()
                                .fold(U256::zero(), |acc, cur| U256::from(acc) + cur.1);

                            let result = self
                                .env()
                                .transfer(item.owner, item.raised + contributors_worth);
                            match result {
                                Ok(_) => {
                                    self.items.set(
                                        id,
                                        &None::<WishListItem>,
                                    );
                                },
                                Err(_) => {
                                    return Err(Error::InvalidContribution);
                                }
                            }
                            self.items.set(id, &None::<WishListItem>);
                        } else {
                            return Err(Error::InvalidContribution);
                        }

                        Ok(())
                    }
                }
            }
        }

        #[ink(message)]
        pub fn get_wishlist_item(&self, id: u32) -> Result<Option<WishListItem>> {
            self.items.get(id).ok_or(Error::WishNotFound)
        }

        pub fn get_caller(&self) -> H160 {
            self.env().caller()
        }

        pub fn get_contributors_raised(&self, id: u32) -> Option<U256> {
            let wishlist = self.items.get(id);

            match wishlist {
                None => None, // return nothing if there is no wishlist
                Some(item) => {
                    if item.is_some() {
                        let item = item.unwrap();
                        let contributors = item.contributors;
                        let total_raised = contributors.iter().fold(U256::zero(), |acc, curr| {
                            acc + curr.1
                        });
                        return Some(total_raised);
                    } else {
                        return None;
                    }
                }
            }
        }
    }

    /// Unit tests in Rust are normally defined within such a `#[cfg(test)]`
    /// module and test functions are marked with a `#[test]` attribute.
    /// The below code is technically just normal Rust code.
    #[cfg(test)]
    mod tests {
        /// Imports all the definitions from the outer scope so we can use them here.
        use super::*;
        use ink::env::test::*;

        /// We test if the default constructor does its job.
        #[ink::test]
        fn default_works() {
            let wishlist = Wishlist::default();
            assert_eq!(wishlist.next_item_id, 1_u32);
        }

        /// The transferred_value must be greater than 10% of the target amount
        #[ink::test]
        fn add_wishlist_value_must_match_target() {
            let mut wishlist = Wishlist::default();
            let target = U256::from(1000);
            let end_date = 1752797402; // Example timestamp
            let description = String::from("Test Wishlist");

            // No transfer value should fail
            let result = wishlist.add_wishlist_item(description, end_date, target);
            assert!(result.is_err(), "Expected error for no transfer value");
            assert!(result.err() == Some(Error::InvalidContribution));

            // Pass value less that 10% target
            let description = String::from("Test < 10% Wishlist");
            let transfer_value = U256::from(50);
            set_value_transferred(transfer_value);
            let result = wishlist.add_wishlist_item(description, end_date, target);
            assert!(
                result.is_err(),
                "Expected error for insufficient transfer value"
            );
            assert!(result.err() == Some(Error::InvalidContribution));

            let description = String::from("Test >= 10% Wishlist");
            let transfer_value = U256::from(100);
            set_value_transferred(transfer_value);
            let result = wishlist.add_wishlist_item(description, end_date, target);
            assert!(
                result.is_ok(),
                "Expected successful addition of wishlist item"
            );
            assert_eq!(wishlist.next_item_id, 2_u32);
        }

        #[ink::test]
        pub fn add_wishlist_should_succeed() {
            let mut contract = Wishlist::default();
            let description = String::from("Add Wishlist should succeed");
            let end_date = 1752798324779;
            let target = U256::from(1000);

            set_value_transferred(U256::from(115));
            let result = contract.add_wishlist_item(description, end_date, target);

            assert!(result.is_ok(), "is should be Ok");
            assert_eq!(contract.next_item_id, 2_u32);
            let item = contract.get_wishlist_item(0);
            assert!(item.is_ok(), "Item should be found");
            assert_eq!(item.unwrap().unwrap().raised, U256::from(115));
        }

        #[ink::test]
        pub fn fund_wish_should_succeed() {
            let mut wishlist = Wishlist::default();

            set_value_transferred(U256::from(125));
            set_caller(default_accounts().alice);
            let _ = wishlist.add_wishlist_item(
                String::from("Wishlist Item 1"),
                1752798324779,
                U256::from(1000),
            );

            // value_transferred mjst not be 0
            set_value_transferred(U256::zero());
            let result = wishlist.fund_wish(0);
            assert!(result.is_err(), "Funding will not succeed");
            assert_eq!(result.err(), Some(Error::InvalidContribution));

            // ID must exist
            set_value_transferred(U256::from(10));
            let result = wishlist.fund_wish(1);
            assert!(result.is_err(), "ID must exist");
            assert_eq!(result.err(), Some(Error::WishNotFound));

            set_value_transferred(U256::from(10));
            let result = wishlist.fund_wish(0);
            assert!(result.is_ok(), "Funding should succeed");
            assert_eq!(wishlist.next_item_id, 2_u32);
            assert_eq!(
                wishlist.get_wishlist_item(0).unwrap().unwrap().raised,
                U256::from(135)
            );

            set_caller(default_accounts().bob);
            let result = wishlist.fund_wish(0);
            assert!(result.is_ok(), "Funding should succeed");
            assert_eq!(
                wishlist.get_wishlist_item(0).unwrap().unwrap().raised,
                U256::from(135)
            );
            assert_eq!(
                wishlist
                    .get_wishlist_item(0)
                    .unwrap()
                    .unwrap()
                    .contributors
                    .len(),
                1
            );
            assert_eq!(
                wishlist.get_wishlist_item(0).unwrap().unwrap().contributors[0].0,
                default_accounts().bob
            );
        }

        #[ink::test]
        pub fn claim_wish_fail_for_error_conditions() {
            let mut wishlist = Wishlist::default();

            set_caller(default_accounts().alice);
            set_value_transferred(U256::from(120));
            let _ = wishlist.add_wishlist_item(
                String::from("Wishlist Item"),
                1752800402,
                U256::from(1000),
            );
            set_value_transferred(U256::from(100));
            let _ = wishlist.fund_wish(0);

            set_caller(default_accounts().alice);
            advance_block::<ink::env::DefaultEnvironment>();
            set_block_timestamp::<ink::env::DefaultEnvironment>(1752800500);
            let result = wishlist.claim_wish(0);
            assert!(result.is_err(), "Claiming wish should fail");
            assert_eq!(result.err(), Some(Error::InvalidContribution));
        }

        #[ink::test]
        pub fn claim_wish_should_succeed() {
            let mut wishlist = Wishlist::default();
            set_caller(default_accounts().alice);
            set_value_transferred(U256::from(2));
            let _ = wishlist.add_wishlist_item(
                String::from("Wishlist Item"),
                1752800402,
                U256::from(3)
            );

            advance_block::<ink::env::DefaultEnvironment>();
            set_caller(default_accounts().alice);
            set_value_transferred(U256::from(2));
            let _ = wishlist.fund_wish(0);

            advance_block::<ink::env::DefaultEnvironment>();
            set_block_timestamp::<ink::env::DefaultEnvironment>(1752800500);
            let result = wishlist.claim_wish(0);
            assert!(result.is_ok(), "Claiming wish should succeed");
            assert_eq!(wishlist.next_item_id, 2_u32);
        }
    }
}
