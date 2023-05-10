/*
 * Copyright (C) 2022  Richard Ulrich
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation; version 3.
 *
 * utwallet is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with this program.  If not, see <http://www.gnu.org/licenses/>.
 */

use regex::Regex;

pub enum InputEval {
    Mainnet(String),
    Lightning(String),
}

impl InputEval {
    pub fn evaluate(addr: &str) -> Result<Self, String> {
        let rgx_btc_addr = r#"(bc1|[13])[a-zA-HJ-NP-Z0-9]{25,39}"#;

        let re = Regex::new(&format!("^{}$", rgx_btc_addr)).map_err(|e| e.to_string())?;
        if re.is_match(addr) {
            return Ok(Self::Mainnet(addr.to_string()));
        }

        // https://developer.bitcoin.org/devguide/payment_processing.html
        let re = Regex::new(&format!(
            "^bitcoin:{}([?&](amount|label|message)=([^&]+))*$",
            rgx_btc_addr
        ))
        .map_err(|e| e.to_string())?;
        if re.is_match(addr) {
            return Ok(Self::Mainnet(addr.to_string()));
        }

        // https://www.bolt11.org/
        let rgx_bolt11 = r#"^lnbc[a-z0-9]{100,700}$"#;
        let re = Regex::new(&rgx_bolt11).map_err(|e| e.to_string())?;
        if re.is_match(addr) {
            return Ok(Self::Lightning(addr.to_string()));
        }

        // https://bolt12.org/
        let rgx_bolt12 = r#"^lno1[a-z0-9]{55,150}$"#;
        let re = Regex::new(&rgx_bolt12).map_err(|e| e.to_string())?;
        if re.is_match(addr) {
            return Ok(Self::Lightning(addr.to_string()));
        }

        /*
                let captures = re.captures(addr).map(|captures| {
                    captures
                        .iter()
                        .skip(1)
                        .take(3)
                        .flatten()
                        .map(|c| c.as_str())
                        .collect::<Vec<_>>()
                });
        */
        Err("Unknown input format".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[should_panic(expected = "Unknown input format")]
    fn test_empty() {
        let inp = "";
        let _resp = InputEval::evaluate(inp).unwrap();
    }

    #[test]
    #[should_panic(expected = "Unknown input format")]
    fn test_short_numeric() {
        let inp = "1234567890";
        let _resp = InputEval::evaluate(inp).unwrap();
    }

    #[test]
    fn test_legacy_address() {
        let inp = "3M5f673Ler6iJbatJNvex7EYANRsydSQXE";
        let resp = InputEval::evaluate(inp).unwrap();
        if let InputEval::Mainnet(addr) = resp {
            assert_eq!(inp, addr);
        } else {
            panic!("not recognized as regular mainnet address");
        }
    }

    #[test]
    fn test_beech_address() {
        let inp = "bc1qa8dn66xn2yq4fcaee4f0gwkkr6e6em643cm8fa";
        let resp = InputEval::evaluate(inp).unwrap();
        if let InputEval::Mainnet(addr) = resp {
            assert_eq!(inp, addr);
        } else {
            panic!("not recognized as regular mainnet address");
        }
    }

    #[test]
    fn test_uri_amount() {
        let inp = "bitcoin:bc1qa8dn66xn2yq4fcaee4f0gwkkr6e6em643cm8fa?amount=100";
        let resp = InputEval::evaluate(inp).unwrap();
        if let InputEval::Mainnet(addr) = resp {
            assert_eq!(inp, addr);
        } else {
            panic!("not recognized as regular mainnet address");
        }
    }

    #[test]
    fn test_bolt11_short() {
        let inp = "lnbc1pjzg3y4sp5t5pqc4w2re6duurq9smwhd78688rwmg2hwxhypxn0vqgu9vgjxnspp5z7p6kn5fpnr8zefvhdw90gascnae5a9s2flrwjp45a6tf53gwrrqdq9u2d2zxqr3jscqpjrzjqvp62xyytkuen9rc8asxue3fuuzultc89ewwnfxch70zf80yl0gpjzxypyqqxhqqqqqqqqqqqqqqqzqq9q9qx3qysgqcnwt6hdzlz3r5k3vqlwcyjrgmyyxrcq7rv304w32q8s6zqe4r7vjvvqxq8rk0g8j9udljtr9dw908ye7608z945gpa3h0avudrqtcpsp7zd4mp";
        let resp = InputEval::evaluate(inp).unwrap();
        if let InputEval::Lightning(invoice) = resp {
            assert_eq!(inp, invoice);
        } else {
            panic!("not recognized as lightning invoice");
        }
    }

    #[test]
    fn test_bolt11_long() {
        let inp = "lnbc3518772650p1pjzg3x2sp59yemkg0cfmsxmugaesm304av4cx4mrp8q7zl65sses7dya7v725spp52ezaxjly2cvdvzlnyakgrq8v3gpnc58rtjepwch74gwgx05snvvqd2qw3jhxapqw3jhxapqw3jhxapqw3jhxapqw3jhxapqw3jhxapqw3jhxapqw3jhxapqw3jhxapqw3jhxapqw3jhxapqw3jhxapqw3jhxapqw3jhxapqw3jhxapqw3jhxapqw3jhxapqw3jhxapqw3jhxapqw3jhxapqw3jhxapqw3jhxapqw3jhxapqw3jhxapqw3jhxapqw3jhxapqw3jhxapqw3jhxapqw3jhxapqw3jhxapqw3jhxapqw3jhxapqw3jhxapqw3jhxapqw3jhxapqw3jhxapqw3jhxapqw3jhxapqw3jhxapqw3jhxapqxqr3jscqpjrzjq032f2wvt88a4lpgxa3nlxuuzd6xmm5azq8np92afzqnsfvv09qk6za0p5qqjdgqqqqqqqqqqqqqqqqqyu9qx3qysgq8v099gx9mlh9fvs3l0n0qlgka7kt0en8kca659maxy3kuww9y4l3utddc3yrx24hs2jwfyx8h0w2t6xltetqzd4a0mlpqwjz2mp5stsqvat45l";
        let resp = InputEval::evaluate(inp).unwrap();
        if let InputEval::Lightning(invoice) = resp {
            assert_eq!(inp, invoice);
        } else {
            panic!("not recognized as lightning invoice");
        }
    }

    #[test]
    fn test_bolt12_short() {
        let inp = "lno1pgqpvggr53478rgx3s4uttelcy76ssrepm2kg0ead5n7tc6dvlkj4mqkeens";
        let resp = InputEval::evaluate(inp).unwrap();
        if let InputEval::Lightning(invoice) = resp {
            assert_eq!(inp, invoice);
        } else {
            panic!("not recognized as lightning invoice");
        }
    }

    #[test]
    fn test_bolt12_long() {
        let inp = "lno1pqpzwrc2936x2um5yp6x2um5yp6x2um5yp6x2um5yp6x2um5yp6x2um5yp6x2um5yp6x2um5yp6x2um5zcss8frtuwxsdrptckhnlsfa4pq8jrk4vsln6mf8uh356eld9tkpdnn8";
        let resp = InputEval::evaluate(inp).unwrap();
        if let InputEval::Lightning(invoice) = resp {
            assert_eq!(inp, invoice);
        } else {
            panic!("not recognized as lightning invoice");
        }
    }
}
