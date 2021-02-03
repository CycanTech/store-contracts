#![cfg_attr(not(feature = "std"), no_std)]

use ink_babe_random::{CustomEnvironment, Randomness};
use ink_lang as ink;

#[ink::contract(env = crate::CustomEnvironment)]
mod patralottery {
    use crate::Randomness;
    use core::fmt::Write;
    use ink_prelude::{string::String, vec, vec::Vec};
    use ink_storage::{
        collections::HashMap as StorageMap,
        traits::{PackedLayout, SpreadLayout},
    };

    pub const DOTS: Balance = 10_000_000_000;

    pub type EpochID = u64;

    #[derive(
        Debug, PartialEq, Eq, scale::Encode, scale::Decode, Clone, Copy, SpreadLayout, PackedLayout,
    )]
    #[cfg_attr(
        feature = "std",
        derive(scale_info::TypeInfo, ink_storage::traits::StorageLayout)
    )]
    pub enum Rank {
        None,
        FirstPrize,
        SecondPrize,
        ThirdPrize,
    }

    #[ink(event)]
    pub struct BuyTickets {
        #[ink(topic)]
        ticket_num: Vec<u32>,
        #[ink(topic)]
        amount: u32,
        #[ink(topic)]
        epoch: EpochID,
    }

    #[ink(event)]
    pub struct DrawLottery {
        #[ink(topic)]
        epoch: EpochID,
        #[ink(topic)]
        randomness: Hash,
        #[ink(topic)]
        win_num: Vec<u32>,
    }

    #[derive(
        Debug, PartialEq, Eq, Clone, scale::Encode, scale::Decode, SpreadLayout, PackedLayout,
    )]
    #[cfg_attr(
        feature = "std",
        derive(scale_info::TypeInfo, ink_storage::traits::StorageLayout)
    )]
    pub struct Lottery {
        pub random: Hash,
        pub win_num: Vec<u32>,
        pub buyers: Vec<AccountId>,
        pub end: bool,
    }

    #[derive(
        Debug, PartialEq, Eq, Clone, scale::Encode, scale::Decode, SpreadLayout, PackedLayout,
    )]
    #[cfg_attr(
        feature = "std",
        derive(scale_info::TypeInfo, ink_storage::traits::StorageLayout)
    )]
    pub struct Tickets {
        pub num: Vec<u32>,
        pub amount: u32,
        pub reward: Balance,
        pub rank: Rank,
    }

    #[ink(storage)]
    pub struct PatraLottery {
        epochs: StorageMap<EpochID, Lottery>,
        players: StorageMap<(EpochID, AccountId), Vec<Tickets>>,
        buyers: StorageMap<AccountId, Vec<EpochID>>,
        reward_pool: Balance,
        epoch: EpochID,
        start_slot: u64,
        duration: u64,
    }

    impl PatraLottery {
        #[ink(constructor)]
        pub fn new() -> Self {
            let ret: Randomness = Self::env().extension().random("".as_bytes()).unwrap();
            Self {
                epochs: StorageMap::new(),
                players: StorageMap::new(),
                buyers: StorageMap::new(),
                reward_pool: 0,
                epoch: ret.epoch + 1,
                start_slot: ret.start_slot,
                duration: ret.duration,
            }
        }

        #[ink(message, payable)]
        pub fn buy_tickets(&mut self, epoch_id: EpochID, num: Vec<u32>, amount: u32) {
            let ret: Randomness = self.env().extension().random("".as_bytes()).unwrap();
            assert!(epoch_id >= ret.epoch + 1);
            let caller = self.env().caller();
            let spend = self.env().transferred_balance();
            assert!(spend >= DOTS * amount as u128);
            assert_eq!(num.len(), 3);

            self.reward_pool += spend;

            // update epochs
            if let Some(epoch) = self.epochs.get_mut(&epoch_id) {
                if !epoch.buyers.contains(&caller) {
                    epoch.buyers.push(caller);
                }
            } else {
                self.epochs.insert(
                    epoch_id,
                    Lottery {
                        random: Default::default(),
                        win_num: vec![],
                        buyers: vec![caller],
                        end: false,
                    },
                );
            }

            // update players
            let ticket = Tickets {
                num: num.clone(),
                amount,
                reward: 0,
                rank: Rank::None,
            };
            if let Some(tic) = self.players.get_mut(&(self.epoch, caller)) {
                tic.push(ticket);
            } else {
                self.players.insert((self.epoch, caller), vec![ticket]);
            }

            // update buyers
            if let Some(eps) = self.buyers.get_mut(&caller) {
                if !eps.contains(&epoch_id) {
                    eps.push(epoch_id);
                }
            } else {
                self.buyers.insert(caller, vec![epoch_id]);
            }

            self.env().emit_event(BuyTickets {
                ticket_num: num,
                amount,
                epoch: epoch_id,
            })
        }

        #[ink(message)]
        pub fn draw_lottery(&mut self, epoch_id: EpochID) {
            let ret: Randomness = self.env().extension().random("".as_bytes()).unwrap();
            let epoch = ret.epoch;
            assert_eq!(epoch, epoch_id);
            let info = self.epochs.get_mut(&epoch).unwrap();
            assert!(!info.end);

            let (_hex_random, win_num) = Self::get_winning_number(ret.randomness);
            info.end = true;
            info.random = ret.randomness;
            info.win_num = win_num.clone();

            // claim reward
            let caller = self.env().caller();
            // 0.1 DOT
            self.env().transfer(caller, DOTS / 10).unwrap();
            self.reward_pool -= DOTS / 10;

            if let Some(lottery) = self.epochs.get(&epoch) {
                let mut first_count = 0_u32;
                let mut first_palyers: Vec<AccountId> = Vec::new();
                for buyer in lottery.buyers.iter() {
                    if let Some(player) = self.players.get_mut(&(epoch, *buyer)) {
                        for tic in player {
                            let rank = Self::rank(tic.num.clone(), win_num.clone());
                            match rank {
                                Rank::FirstPrize => {
                                    tic.rank = Rank::FirstPrize;
                                    first_count += tic.amount;
                                    first_palyers.push(*buyer);
                                }
                                Rank::SecondPrize => {
                                    tic.rank = Rank::SecondPrize;
                                    let reward = DOTS * 10 * tic.amount as u128;
                                    if self.reward_pool > 0 {
                                        if self.reward_pool > reward {
                                            Self::env().transfer(*buyer, reward).unwrap();
                                            tic.reward = reward;
                                        } else {
                                            Self::env().transfer(*buyer, self.reward_pool).unwrap();
                                            tic.reward = self.reward_pool;
                                        }
                                        self.reward_pool = self.reward_pool.saturating_sub(reward);
                                    }
                                }
                                Rank::ThirdPrize => {
                                    tic.rank = Rank::ThirdPrize;
                                    let reward = DOTS * 2 * tic.amount as u128;
                                    if self.reward_pool > 0 {
                                        if self.reward_pool > reward {
                                            Self::env().transfer(*buyer, reward).unwrap();
                                            tic.reward = reward;
                                        } else {
                                            Self::env().transfer(*buyer, self.reward_pool).unwrap();
                                            tic.reward = self.reward_pool;
                                        }
                                        self.reward_pool = self.reward_pool.saturating_sub(reward);
                                    }
                                }
                                _ => (),
                            }
                        }
                    }
                }

                if self.reward_pool > 0 {
                    let reward = self.reward_pool / first_count as u128;
                    for player in first_palyers.iter() {
                        let tickets = self.players.get_mut(&(epoch_id, *player)).unwrap();
                        for tic in tickets.iter_mut() {
                            if tic.rank == Rank::FirstPrize {
                                Self::env().transfer(*player, reward).unwrap();
                                tic.reward = reward;
                            }
                        }
                    }
                    self.reward_pool = 0;
                }
            }
            self.env().emit_event(DrawLottery {
                epoch,
                randomness: ret.randomness,
                win_num,
            })
        }

        fn rank(numbers: Vec<u32>, win_num: Vec<u32>) -> Rank {
            assert_eq!(numbers.len(), 3);
            let count = win_num
                .iter()
                .zip(numbers.iter())
                .filter(|(x, y)| **x == **y)
                .count();
            match count {
                3 => Rank::FirstPrize,
                2 => Rank::SecondPrize,
                1 => Rank::ThirdPrize,
                _ => Rank::None,
            }
        }

        #[ink(message)]
        pub fn winning_number(&self, subject: Vec<u8>) -> (String, Vec<u32>) {
            let ret: Randomness = self.env().extension().random(subject.as_slice()).unwrap();
            Self::get_winning_number(ret.randomness)
        }
    }

    impl PatraLottery {
        pub fn get_winning_number(random: Hash) -> (String, Vec<u32>) {
            let mut seed = String::new();
            for byte in random.as_ref() {
                write!(&mut seed, "{:x}", byte).expect("Unable to write");
            }
            let mut win: Vec<u32> = vec![];
            for (n, v) in seed.chars().filter_map(|x| x.to_digit(10)).enumerate() {
                if n < 3 {
                    win.push(v);
                }
            }
            (seed, win)
        }
    }
}
