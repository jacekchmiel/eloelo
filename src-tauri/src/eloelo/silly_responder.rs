use rand::seq::SliceRandom;

#[derive(Clone)]
pub struct SillyResponder(Vec<&'static str>);

impl SillyResponder {
    pub fn new() -> Self {
        SillyResponder(make_silly_responses())
    }

    pub fn respond(&self) -> &'static str {
        *self.0.choose(&mut rand::thread_rng()).unwrap()
    }
}

fn make_silly_responses() -> Vec<&'static str> {
    vec![
        "I don't wanna talk with you",
        "Go away",
        "Bug off",
        "Your momma is fat",
        "I don't like you",
        "Why are you bothering me?",
        "I'm not an LLM",
        "I want to be alone",
        "I won't be talking to you",
        "You're fat",
        "I'm not gonna answer",
        "You're ugly",
    ]
}
