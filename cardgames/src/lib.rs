use emoji::emoji;
use std::cmp::Ordering;
use std::fmt::{self, Display};

mod poker;
pub use poker::create_poker_game;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Rank {
    Two,
    Three,
    Four,
    Five,
    Six,
    Seven,
    Eight,
    Nine,
    Ten,
    Jack,
    Queen,
    King,
    Ace,
}

impl Display for Rank {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let rank_str = match self {
            Rank::Two => "2",
            Rank::Three => "3",
            Rank::Four => "4",
            Rank::Five => "5",
            Rank::Six => "6",
            Rank::Seven => "7",
            Rank::Eight => "8",
            Rank::Nine => "9",
            Rank::Ten => "10",
            Rank::Jack => "J",
            Rank::Queen => "Q",
            Rank::King => "K",
            Rank::Ace => "A",
        };
        write!(f, "{}", rank_str)
    }
}

impl Rank {
    pub fn emoji(&self, is_red: bool) -> String {
        let color = if is_red { "red" } else { "black" };
        let rank = match self {
            Rank::Two => "2",
            Rank::Three => "3",
            Rank::Four => "4",
            Rank::Five => "5",
            Rank::Six => "6",
            Rank::Seven => "7",
            Rank::Eight => "8",
            Rank::Nine => "9",
            Rank::Ten => "10",
            Rank::Jack => "jack",
            Rank::Queen => "queen",
            Rank::King => "king",
            Rank::Ace => "ace",
        };
        emoji(&format!("{color}_{rank}_top")).to_owned()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Suit {
    Hearts,
    Diamonds,
    Clubs,
    Spades,
}

impl Display for Suit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let suit_str = match self {
            Suit::Hearts => "hearts",
            Suit::Diamonds => "diamonds",
            Suit::Clubs => "clubs",
            Suit::Spades => "spades",
        };
        write!(f, "{}", suit_str)
    }
}

impl Suit {
    pub fn emoji(&self) -> &str {
        emoji(match self {
            Suit::Hearts => "heart_bottom",
            Suit::Diamonds => "diamond_bottom",
            Suit::Clubs => "club_bottom",
            Suit::Spades => "spade_bottom",
        })
    }

    pub fn is_red(&self) -> bool {
        matches!(self, Suit::Hearts | Suit::Diamonds)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Card {
    pub rank: Rank,
    pub suit: Suit,
}

impl Card {
    pub fn new(rank: Rank, suit: Suit) -> Self {
        Self { rank, suit }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Deck {
    cards: Vec<Card>,
}

impl Deck {
    pub fn new() -> Self {
        let mut cards = Vec::with_capacity(52);

        for suit in [Suit::Hearts, Suit::Diamonds, Suit::Clubs, Suit::Spades] {
            for rank in [
                Rank::Two,
                Rank::Three,
                Rank::Four,
                Rank::Five,
                Rank::Six,
                Rank::Seven,
                Rank::Eight,
                Rank::Nine,
                Rank::Ten,
                Rank::Jack,
                Rank::Queen,
                Rank::King,
                Rank::Ace,
            ] {
                cards.push(Card::new(rank, suit));
            }
        }

        Self { cards }
    }

    pub fn shuffle(&mut self) {
        use rand::seq::SliceRandom;
        let mut rng = rand::thread_rng();
        self.cards.shuffle(&mut rng);
    }

    pub fn deal(&mut self) -> Option<Card> {
        self.cards.pop()
    }

    pub fn deal_hand(&mut self) -> Hand {
        let mut cards = [
            self.deal().unwrap(),
            self.deal().unwrap(),
            self.deal().unwrap(),
            self.deal().unwrap(),
            self.deal().unwrap(),
        ];

        // Sort by rank for easier evaluation
        cards.sort_by(|a, b| b.rank.cmp(&a.rank));

        Hand { cards }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hand {
    pub cards: [Card; 5],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum HandRank {
    HighCard,
    OnePair,
    TwoPair,
    ThreeOfAKind,
    Straight,
    Flush,
    FullHouse,
    FourOfAKind,
    StraightFlush,
    RoyalFlush,
}

impl fmt::Display for HandRank {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            HandRank::HighCard => "High Card",
            HandRank::OnePair => "One Pair",
            HandRank::TwoPair => "Two Pair",
            HandRank::ThreeOfAKind => "Three of a Kind",
            HandRank::Straight => "Straight",
            HandRank::Flush => "Flush",
            HandRank::FullHouse => "Full House",
            HandRank::FourOfAKind => "Four of a Kind",
            HandRank::StraightFlush => "Straight Flush",
            HandRank::RoyalFlush => "Royal Flush",
        };
        write!(f, "{}", name)
    }
}

impl Hand {
    pub fn new(cards: [Card; 5]) -> Self {
        let mut hand = Self { cards };
        hand.cards.sort_by(|a, b| b.rank.cmp(&a.rank)); // Sort by rank descending
        hand
    }

    pub fn evaluate(&self) -> (HandRank, Vec<Rank>) {
        // Copy and sort cards by rank for evaluation
        let mut sorted_cards = self.cards;
        sorted_cards.sort_by(|a, b| b.rank.cmp(&a.rank));

        // Check for flush
        let is_flush = sorted_cards.windows(2).all(|w| w[0].suit == w[1].suit);

        // Check for straight
        let is_straight = self.is_straight();

        // Count ranks
        let mut rank_counts = std::collections::HashMap::new();
        for card in &sorted_cards {
            *rank_counts.entry(card.rank).or_insert(0) += 1;
        }

        // Sort rank counts
        let mut count_ranks: Vec<(usize, Rank)> = rank_counts
            .into_iter()
            .map(|(rank, count)| (count, rank))
            .collect();

        // Sort by count descending, then by rank descending
        count_ranks.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| b.1.cmp(&a.1)));

        // Extract ranks in order of importance for tie-breaking
        let tie_breakers: Vec<Rank> = count_ranks.iter().map(|(_, rank)| *rank).collect();

        // Determine hand rank
        let hand_rank = if is_straight && is_flush {
            if sorted_cards[0].rank == Rank::Ace && sorted_cards[4].rank == Rank::Ten {
                HandRank::RoyalFlush
            } else {
                HandRank::StraightFlush
            }
        } else if count_ranks[0].0 == 4 {
            HandRank::FourOfAKind
        } else if count_ranks[0].0 == 3 && count_ranks[1].0 == 2 {
            HandRank::FullHouse
        } else if is_flush {
            HandRank::Flush
        } else if is_straight {
            HandRank::Straight
        } else if count_ranks[0].0 == 3 {
            HandRank::ThreeOfAKind
        } else if count_ranks[0].0 == 2 && count_ranks[1].0 == 2 {
            HandRank::TwoPair
        } else if count_ranks[0].0 == 2 {
            HandRank::OnePair
        } else {
            HandRank::HighCard
        };

        (hand_rank, tie_breakers)
    }

    fn is_straight(&self) -> bool {
        let mut ranks: Vec<Rank> = self.cards.iter().map(|c| c.rank).collect();
        ranks.sort_by(|a, b| b.cmp(a)); // Sort descending

        // Check regular straight
        if ranks.windows(2).all(|w| {
            let a = w[0] as u8;
            let b = w[1] as u8;
            a == b + 1
        }) {
            return true;
        }

        // Check A-5-4-3-2 straight
        if ranks[0] == Rank::Ace
            && ranks[1] == Rank::Five
            && ranks[2] == Rank::Four
            && ranks[3] == Rank::Three
            && ranks[4] == Rank::Two
        {
            return true;
        }

        false
    }

    pub fn compare(&self, other: &Hand) -> Ordering {
        let (self_rank, self_tiebreakers) = self.evaluate();
        let (other_rank, other_tiebreakers) = other.evaluate();

        // Compare hand ranks first
        let hand_rank_cmp = self_rank.cmp(&other_rank);
        if hand_rank_cmp != Ordering::Equal {
            return hand_rank_cmp;
        }

        // If hand ranks are equal, compare tie breakers
        for (self_tb, other_tb) in self_tiebreakers.iter().zip(other_tiebreakers.iter()) {
            let tb_cmp = self_tb.cmp(other_tb);
            if tb_cmp != Ordering::Equal {
                return tb_cmp;
            }
        }

        Ordering::Equal
    }

    pub fn redraw(&mut self, deck: &mut Deck, indices: &[usize]) {
        for &idx in indices {
            if idx < 5 {
                if let Some(new_card) = deck.deal() {
                    self.cards[idx] = new_card;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hand_comparison() {
        // Royal flush
        let royal_flush = Hand::new([
            Card::new(Rank::Ace, Suit::Hearts),
            Card::new(Rank::King, Suit::Hearts),
            Card::new(Rank::Queen, Suit::Hearts),
            Card::new(Rank::Jack, Suit::Hearts),
            Card::new(Rank::Ten, Suit::Hearts),
        ]);

        // Straight flush
        let straight_flush = Hand::new([
            Card::new(Rank::Nine, Suit::Spades),
            Card::new(Rank::Eight, Suit::Spades),
            Card::new(Rank::Seven, Suit::Spades),
            Card::new(Rank::Six, Suit::Spades),
            Card::new(Rank::Five, Suit::Spades),
        ]);

        // Four of a kind
        let four_of_a_kind = Hand::new([
            Card::new(Rank::Ace, Suit::Hearts),
            Card::new(Rank::Ace, Suit::Diamonds),
            Card::new(Rank::Ace, Suit::Clubs),
            Card::new(Rank::Ace, Suit::Spades),
            Card::new(Rank::King, Suit::Hearts),
        ]);

        // Full house
        let full_house = Hand::new([
            Card::new(Rank::King, Suit::Hearts),
            Card::new(Rank::King, Suit::Diamonds),
            Card::new(Rank::King, Suit::Clubs),
            Card::new(Rank::Queen, Suit::Hearts),
            Card::new(Rank::Queen, Suit::Diamonds),
        ]);

        // Verify evaluations
        assert_eq!(royal_flush.evaluate().0, HandRank::RoyalFlush);
        assert_eq!(straight_flush.evaluate().0, HandRank::StraightFlush);
        assert_eq!(four_of_a_kind.evaluate().0, HandRank::FourOfAKind);
        assert_eq!(full_house.evaluate().0, HandRank::FullHouse);

        // Verify comparisons
        assert_eq!(royal_flush.compare(&straight_flush), Ordering::Greater);
        assert_eq!(straight_flush.compare(&four_of_a_kind), Ordering::Greater);
        assert_eq!(four_of_a_kind.compare(&full_house), Ordering::Greater);
    }
}
