use gravity::result_errors::host;

wit_bindgen::generate!({
    world: "result-errors",
});

struct ResultErrors;

export!(ResultErrors);

fn refusal(n: u32) -> Refusal {
    match n % 4 {
        0 => Refusal::NoCredential,
        1 => Refusal::Malformed,
        2 => Refusal::UntypedPeer,
        _ => Refusal::ForeignDomain,
    }
}

impl Guest for ResultErrors {
    fn decide(n: u32) -> Result<Decided, Refusal> {
        if n < 4 {
            Err(refusal(n))
        } else {
            Ok(Decided {
                name: format!("n{n}"),
                n,
            })
        }
    }

    fn inspect(n: u32) -> Result<u32, Problem> {
        if n.is_multiple_of(2) {
            Ok(n * 2)
        } else {
            Err(Problem {
                code: n,
                detail: "odd".into(),
            })
        }
    }

    fn classify(n: u32) -> Result<String, Fault> {
        match n {
            0 => Err(Fault::Refused(Refusal::ForeignDomain)),
            1 => Err(Fault::Problem(Problem {
                code: 1,
                detail: "p".into(),
            })),
            2 => Err(Fault::Message("m".into())),
            3 => Err(Fault::Unknown),
            _ => Ok("fine".into()),
        }
    }

    fn only_err(n: u32) -> Result<(), Refusal> {
        if n == 0 { Ok(()) } else { Err(refusal(n)) }
    }

    fn via_check(n: u32) -> Result<u32, Refusal> {
        host::check(n)
    }

    fn via_probe(n: u32) -> Result<(), Problem> {
        host::probe(n)
    }

    fn via_explain(n: u32) -> Result<String, Fault> {
        host::explain(n)
    }
}
