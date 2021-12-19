/// Converts a number from 0-25 to a alphabet letter
fn int2letter(num: u128) -> Result<char, u128> {
    let conv = match num {
        0 => 'A',
        1 => 'B',
        2 => 'C',
        3 => 'D',
        4 => 'E',
        5 => 'F',
        6 => 'G',
        7 => 'H',
        8 => 'I',
        9 => 'J',
        10 => 'K',
        11 => 'L',
        12 => 'M',
        13 => 'N',
        14 => 'O',
        15 => 'P',
        16 => 'Q',
        17 => 'R',
        18 => 'S',
        19 => 'T',
        20 => 'U',
        21 => 'V',
        22 => 'W',
        23 => 'X',
        24 => 'Y',
        25 => 'Z',
        _ => ' '
    };
    if conv == ' ' {
        return Err(num);
    } else {
        return Ok(conv);
    };
}

fn letter2int(ltr: char) -> Result<u8, char> {
    let conv = match ltr {
        'A' => 0,
        'B' => 1,
        'C' => 2,
        'D' => 3,
        'E' => 4,
        'F' => 5,
        'G' => 6,
        'H' => 7,
        'I' => 8,
        'J' => 9,
        'K' => 10,
        'L' => 11,
        'M' => 12,
        'N' => 13,
        'O' => 14,
        'P' => 15,
        'Q' => 16,
        'R' => 17,
        'S' => 18,
        'T' => 19,
        'U' => 20,
        'V' => 21,
        'W' => 22,
        'X' => 23,
        'Y' => 24,
        'Z' => 25,
        _ => 50
    };
    if conv == 50 {
        return Err(ltr);
    } else {
        return Ok(conv);
    }
}

fn num_to_code(mut num: u128) -> String {
    let mut res = String::new();
    loop {
        let dig = int2letter(num % 25);
        res.push(dig.unwrap());
        num /= 25;
        if num <= 0 {break;}
    }
    return res.chars().rev().collect::<String>();
}

fn code_to_num(mut code: String) -> u128 {
    let mut res: u128 = 0;
    code.make_ascii_uppercase();
    for ch in code.chars() {
        let v = letter2int(ch).unwrap();
        res *= 25;
        res += v as u128;
    };
    res
}

pub fn ip_to_code(ip: std::net::SocketAddrV4) -> String {
    let addr = ip.ip();
    let pts = addr.octets();
    let a = pts[0];
    let b = pts[1];
    let c = pts[2];
    let d = pts[3];
    let p = ip.port();
    let mut combined: u128 = 0;
    combined += a as u128;
    combined += b as u128 * 1000;
    combined += c as u128 * 1000000;
    combined += d as u128 * 1000000000;
    combined += p as u128 * 1000000000000;
    num_to_code(combined)
}

pub fn code_to_ip(code: String) -> std::net::SocketAddrV4 {
    let mut combined = code_to_num(code);
    let p = combined / 1000000000000;
    combined -= p * 1000000000000;
    let d = combined / 1000000000;
    combined -= d * 1000000000;
    let c = combined / 1000000;
    combined -= c * 1000000;
    let b = combined / 1000;
    combined -= b * 1000;
    let a = combined;
    return std::net::SocketAddrV4::new(
        std::net::Ipv4Addr::new(
            a.try_into().unwrap(),
            b.try_into().unwrap(),
            c.try_into().unwrap(),
            d.try_into().unwrap()
        ),
        p.try_into().unwrap()
    );
}