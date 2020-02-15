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

#[riko::fun]
fn struct_(love: crate::serde::Love, work: crate::serde::Work) -> crate::serde::Life {
    Life {
        happy: !love.target.is_empty() && work.salary > 0,
    }
}
