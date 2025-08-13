use rand::distr::Alphanumeric;
use rand::Rng;

pub fn random_seed() -> String {
    rand::rng().sample_iter(&Alphanumeric).take(8).map(char::from).collect()
}
