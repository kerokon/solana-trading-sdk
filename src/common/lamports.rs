use serde::de::{Error as DeError, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;
use std::ops::{Add, Div, Sub};

const SOL_TO_LAMPORTS_FACTOR: f64 = 1_000_000_000.0;

// --- Newtype for Lamports that handles SOL (f64) input ---
#[derive(Debug, Clone, Hash, Copy, PartialEq, Eq, PartialOrd, Ord, Default)] // Add common traits
pub struct Lamports(pub u64); // The actual value is a u64

impl Add for Lamports {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self(self.0 + rhs.0)
    }
}

impl Sub for Lamports {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Self(self.0 - rhs.0)
    }
}

impl Div for Lamports {
    type Output = Self;

    fn div(self, rhs: Self) -> Self::Output {
        Self(self.0 / rhs.0)
    }
}

impl Lamports {
    // Constructor from SOL (f64) input
    pub fn from_sol(sol_value: f64) -> Result<Self, String> {
        if sol_value < 0.0 {
            return Err(format!("SOL value cannot be negative: {}", sol_value));
        }

        let lamports_f64 = sol_value * SOL_TO_LAMPORTS_FACTOR;

        // Check for overflow after multiplication
        if lamports_f64 > u64::MAX as f64 {
            return Err(format!("Converted lamports value ({}) exceeds u64::MAX", lamports_f64));
        }

        // Round to nearest integer before casting to u64 to handle floating point nuances
        // This is important because 0.000000001 SOL might become 0.999999999 lamports,
        // which truncates to 0. Rounding makes it 1.
        let rounded_lamports = lamports_f64.round();

        Ok(Lamports(rounded_lamports as u64))
    }

    // Convert Lamports back to SOL (f64) for display/serialization
    pub fn to_sol(&self) -> f64 {
        self.0 as f64 / SOL_TO_LAMPORTS_FACTOR
    }
}

impl fmt::Display for Lamports {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

// Implement `Serialize` for Lamports
// When serializing, we want to output the original SOL value (f64)
impl Serialize for Lamports {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        // Serialize as f64 for user-friendly output (e.g., "0.000000001")
        serializer.serialize_f64(self.to_sol())
    }
}

// Implement `Deserialize` for Lamports
// When deserializing, we expect an f64 (SOL) and convert to u64 (Lamports)
impl<'de> Deserialize<'de> for Lamports {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct LamportsVisitor;

        impl<'de> Visitor<'de> for LamportsVisitor {
            type Value = Lamports;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a floating-point number representing SOL")
            }

            // Handle f64 input
            fn visit_f64<E>(self, value: f64) -> Result<Self::Value, E>
            where
                E: DeError,
            {
                Lamports::from_sol(value).map_err(E::custom) // Convert our custom error to Serde's error
            }

            // Also handle integer inputs for convenience, converting them to f64 first
            fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E>
            where
                E: DeError,
            {
                self.visit_f64(value as f64)
            }

            fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
            where
                E: DeError,
            {
                self.visit_f64(value as f64)
            }
        }

        deserializer.deserialize_f64(LamportsVisitor) // Use deserialize_f64 to primarily target f64, visitor handles others
    }
}
