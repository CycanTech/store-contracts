#![cfg_attr(not(feature = "std"), no_std)]

use ink_lang as ink;

#[ink::contract]
mod patrapixel {
    use ink_prelude::{string::String, vec, vec::Vec};

    pub const DOTS: Balance = 10_000_000_000;

    #[ink(event)]
    pub struct PixelUpdate {
        #[ink(topic)]
        creator: AccountId,
    }

    #[ink(storage)]
    pub struct Patrapixel {
        name: String,
        metadata: Vec<u8>,
    }

    impl Patrapixel {
        #[ink(constructor)]
        pub fn new() -> Self {
            Self {
                name: "PatraPixel".parse().unwrap(),
                metadata: vec![0; 160 * 90],
            }
        }

        /// Get and returns pixel metadata
        #[ink(message)]
        pub fn metadata(&self) -> Vec<u8> {
            self.metadata.clone()
        }

        /// update pixel with metadata
        #[ink(message, payable)]
        pub fn update(&mut self, points: Vec<(u32, u8)>) {
            assert!(points.len() > 0);
            assert!(self.env().transferred_balance() >= points.len() as u128 * DOTS);
            points.iter().for_each(|x| {
                if let Some(v) = self.metadata.get_mut(x.0 as usize) {
                    *v = x.1;
                }
            });
            self.env().emit_event(PixelUpdate {
                creator: self.env().caller(),
            });
        }
    }
}
