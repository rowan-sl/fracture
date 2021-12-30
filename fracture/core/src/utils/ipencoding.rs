/// Converts a number from 0-25 to a alphabet letter
const fn int2letter(num: u128) -> Result<char, u128> {
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
        _ => ' ',
    };
    if conv == ' ' {
        Err(num)
    } else {
        Ok(conv)
    }
}

const fn letter2int(ltr: char) -> Result<u8, char> {
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
        _ => 50,
    };
    if conv == 50 {
        Err(ltr)
    } else {
        Ok(conv)
    }
}

#[must_use]
fn num_to_code(mut num: u128) -> Result<String, u128> {
    let mut res = String::new();
    loop {
        let dig = int2letter(num % 25)?;
        res.push(dig);
        num /= 25;
        if num == 0 {
            //cant be less since it is u n s i g n e d you dumbo
            break;
        }
    }
    Ok(res.chars().rev().collect::<String>())
}

#[must_use]
fn code_to_num(mut code: String) -> Result<u128, char> {
    let mut res: u128 = 0;
    code.make_ascii_uppercase();
    for ch in code.chars() {
        let v = letter2int(ch)?;
        res *= 25;
        res += u128::from(v);
    }
    Ok(res)
}

#[must_use]
pub fn ip_to_code(ip: std::net::SocketAddrV4) -> Result<String, u128> {
    let addr = ip.ip();
    let pts = addr.octets();

    let addr_pt1 = pts[0];
    let addr_pt2 = pts[1];
    let addr_pt3 = pts[2];
    let addr_pt4 = pts[3];

    let port = ip.port();

    let mut combined: u128 = 0;
    combined += u128::from(addr_pt4);
    combined += u128::from(addr_pt3) * 1_000;
    combined += u128::from(addr_pt2) * 1_000_000;
    combined += u128::from(addr_pt1) * 1_000_000_000;
    combined += u128::from(port) * 1_000_000_000_000;
    Ok(num_to_code(combined)?)
}

/// Converts a letter code into a socket addr
///
/// # Returns
/// `SocketAddrV4`
///
/// # Panics
/// if the number decoded from the codes cannot be converted into `u8` or `u16` for the address and port respectively, or if there are invalid chars
#[must_use]
pub fn code_to_ip(code: String) -> std::net::SocketAddrV4 {
    let mut combined = code_to_num(code).unwrap();

    let addr_pt1 = combined / 1_000_000_000_000;
    combined -= addr_pt1 * 1_000_000_000_000;

    let addr_pt2 = combined / 1_000_000_000;
    combined -= addr_pt2 * 1_000_000_000;

    let addr_pt3 = combined / 1_000_000;
    combined -= addr_pt3 * 1_000_000;

    let addr_pt4 = combined / 1_000;
    combined -= addr_pt4 * 1_000;

    let port = combined;

    std::net::SocketAddrV4::new(
        std::net::Ipv4Addr::new(
            addr_pt1.try_into().unwrap(),
            addr_pt2.try_into().unwrap(),
            addr_pt3.try_into().unwrap(),
            addr_pt4.try_into().unwrap(),
        ),
        port.try_into().unwrap(),
    )
}

pub enum CodeToIpError {
    Conversion(std::num::TryFromIntError),
    Char(char),
}

impl From<char> for CodeToIpError {
    fn from(item: char) -> Self {
        CodeToIpError::Char(item)
    }
}

impl From<std::num::TryFromIntError> for CodeToIpError {
    fn from(item: std::num::TryFromIntError) -> Self {
        CodeToIpError::Conversion(item)
    }
}

/// Converts a letter code into a socket addr
///
/// # Returns
/// `SocketAddrV4`
///
/// # Errors
/// if the number decoded from the codes cannot be converted into `u8` or `u16` for the address and port respectively
#[must_use]
pub fn code_to_ip_safe(code: String) -> Result<std::net::SocketAddrV4, CodeToIpError> {
    let mut combined = code_to_num(code)?;

    let port = combined / 1_000_000_000_000;
    combined -= port * 1_000_000_000_000;

    let addr_pt1 = combined / 1_000_000_000;
    combined -= addr_pt1 * 1_000_000_000;

    let addr_pt2 = combined / 1_000_000;
    combined -= addr_pt2 * 1_000_000;

    let addr_pt3 = combined / 1_000;
    combined -= addr_pt3 * 1_000;

    let addr_pt4 = combined;

    Ok(std::net::SocketAddrV4::new(
        std::net::Ipv4Addr::new(
            addr_pt1.try_into()?,
            addr_pt2.try_into()?,
            addr_pt3.try_into()?,
            addr_pt4.try_into()?,
        ),
        port.try_into()?,
    ))
}
