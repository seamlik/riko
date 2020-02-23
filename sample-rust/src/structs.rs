use serde::Deserialize;
use serde::Serialize;

#[derive(Serialize, Deserialize)]
pub struct Love {
    pub target: String,
}

#[derive(Serialize, Deserialize)]
pub struct Work {
    pub salary: i64,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Life {
    pub happy: bool,
}

#[riko::fun(marshal = "Struct")]
pub fn structs(
    #[riko::marshal = "Struct"] love: crate::structs::Love,
    #[riko::marshal = "Struct"] work: crate::structs::Work,
) -> crate::structs::Life {
    Life {
        happy: !love.target.is_empty() && work.salary > 0,
    }
}
