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
fn serde_inferred(love: crate::serde::Love, work: crate::serde::Work) -> crate::serde::Life {
    crate::serde::Life {
        happy: !love.target.is_empty() && work.salary > 0,
    }
}

#[riko::fun(marshal = "Struct<crate::serde::Life>")]
fn serde_explicit(
    #[riko::marshal(Struct<crate::serde::Love>)] love: Love,
    #[riko::marshal(Struct<crate::serde::Work>)] work: Work,
) -> crate::serde::Life {
    Life {
        happy: !love.target.is_empty() && work.salary > 0,
    }
}
