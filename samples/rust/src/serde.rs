use serde::Deserialize;
use serde::Serialize;

#[derive(Serialize, Deserialize)]
struct Love {
    pub target: String,
}

#[derive(Serialize, Deserialize)]
struct Work {
    pub salary: u64,
}

#[derive(Serialize, Deserialize)]
struct Life {
    pub happy: bool,
}

#[riko::fun(sig = "(Serde, Serde) -> Serde<Life>")]
fn serde(love: Love, work: Work) -> Life {
    Life {
        happy: !love.target.is_empty() && work.salary > 0,
    }
}
