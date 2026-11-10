//! Vietnamese lunar calendar (âm lịch), after Hồ Ngọc Đức's published
//! algorithm (astronomical new moons and solar terms, UTC+7).

use std::f64::consts::PI;

const TZ: f64 = 7.0;

pub const CAN: [&str; 10] = ["Giáp", "Ất", "Bính", "Đinh", "Mậu", "Kỷ", "Canh", "Tân", "Nhâm", "Quý"];
pub const CHI: [&str; 12] = ["Tý", "Sửu", "Dần", "Mão", "Thìn", "Tỵ", "Ngọ", "Mùi", "Thân", "Dậu", "Tuất", "Hợi"];
pub const TIET_KHI: [&str; 24] = [
    "Xuân phân", "Thanh minh", "Cốc vũ", "Lập hạ", "Tiểu mãn", "Mang chủng", "Hạ chí", "Tiểu thử",
    "Đại thử", "Lập thu", "Xử thử", "Bạch lộ", "Thu phân", "Hàn lộ", "Sương giáng", "Lập đông",
    "Tiểu tuyết", "Đại tuyết", "Đông chí", "Tiểu hàn", "Đại hàn", "Lập xuân", "Vũ thủy", "Kinh trập",
];
pub const MONTH_NAMES: [&str; 12] = [
    "Giêng", "Hai", "Ba", "Tư", "Năm", "Sáu", "Bảy", "Tám", "Chín", "Mười", "Một", "Chạp",
];

fn int(x: f64) -> i64 {
    x.floor() as i64
}

pub fn jd_from_date(dd: i64, mm: i64, yy: i64) -> i64 {
    let a = (14 - mm) / 12;
    let y = yy + 4800 - a;
    let m = mm + 12 * a - 3;
    let mut jd = dd + (153 * m + 2) / 5 + 365 * y + y / 4 - y / 100 + y / 400 - 32045;
    if jd < 2299161 {
        jd = dd + (153 * m + 2) / 5 + 365 * y + y / 4 - 32083;
    }
    jd
}

/// Julian day of the k-th new moon after 1900-01-01.
fn new_moon(k: i64) -> f64 {
    let k = k as f64;
    let t = k / 1236.85;
    let t2 = t * t;
    let t3 = t2 * t;
    let dr = PI / 180.0;
    let mut jd1 = 2415020.75933 + 29.53058868 * k + 0.0001178 * t2 - 0.000000155 * t3;
    jd1 += 0.00033 * ((166.56 + 132.87 * t - 0.009173 * t2) * dr).sin();
    let m = 359.2242 + 29.10535608 * k - 0.0000333 * t2 - 0.00000347 * t3;
    let mpr = 306.0253 + 385.81691806 * k + 0.0107306 * t2 + 0.00001236 * t3;
    let f = 21.2964 + 390.67050646 * k - 0.0016528 * t2 - 0.00000239 * t3;
    let mut c1 = (0.1734 - 0.000393 * t) * (m * dr).sin() + 0.0021 * (2.0 * dr * m).sin();
    c1 = c1 - 0.4068 * (mpr * dr).sin() + 0.0161 * (dr * 2.0 * mpr).sin();
    c1 -= 0.0004 * (dr * 3.0 * mpr).sin();
    c1 = c1 + 0.0104 * (dr * 2.0 * f).sin() - 0.0051 * (dr * (m + mpr)).sin();
    c1 = c1 - 0.0074 * (dr * (m - mpr)).sin() + 0.0004 * (dr * (2.0 * f + m)).sin();
    c1 = c1 - 0.0004 * (dr * (2.0 * f - m)).sin() - 0.0006 * (dr * (2.0 * f + mpr)).sin();
    c1 = c1 + 0.0010 * (dr * (2.0 * f - mpr)).sin() + 0.0005 * (dr * (2.0 * mpr + m)).sin();
    let deltat = if t < -11.0 {
        0.001 + 0.000839 * t + 0.0002261 * t2 - 0.00000845 * t3 - 0.000000081 * t * t3
    } else {
        -0.000278 + 0.000265 * t + 0.000262 * t2
    };
    jd1 + c1 - deltat
}

fn new_moon_day(k: i64) -> i64 {
    int(new_moon(k) + 0.5 + TZ / 24.0)
}

/// Sun longitude in radians at Julian day (fractional).
pub fn sun_longitude_rad(jdn: f64) -> f64 {
    let t = (jdn - 2451545.0) / 36525.0;
    let t2 = t * t;
    let dr = PI / 180.0;
    let m = 357.52910 + 35999.05030 * t - 0.0001559 * t2 - 0.00000048 * t * t2;
    let l0 = 280.46645 + 36000.76983 * t + 0.0003032 * t2;
    let mut dl = (1.914600 - 0.004817 * t - 0.000014 * t2) * (dr * m).sin();
    dl = dl + (0.019993 - 0.000101 * t) * (dr * 2.0 * m).sin() + 0.000290 * (dr * 3.0 * m).sin();
    let mut l = (l0 + dl) * dr;
    l -= PI * 2.0 * (l / (PI * 2.0)).floor();
    l
}

/// Major solar term index 0..11 at the start of local day `day_number`.
fn sun_longitude(day_number: i64) -> i64 {
    int(sun_longitude_rad(day_number as f64 - 0.5 - TZ / 24.0) / PI * 6.0)
}

fn lunar_month11(yy: i64) -> i64 {
    let off = jd_from_date(31, 12, yy) - 2415021;
    let k = int(off as f64 / 29.530588853);
    let mut nm = new_moon_day(k);
    if sun_longitude(nm) >= 9 {
        nm = new_moon_day(k - 1);
    }
    nm
}

fn leap_month_offset(a11: i64) -> i64 {
    let k = int((a11 as f64 - 2415021.076998695) / 29.530588853 + 0.5);
    let mut i = 1;
    let mut arc = sun_longitude(new_moon_day(k + i));
    loop {
        let last = arc;
        i += 1;
        arc = sun_longitude(new_moon_day(k + i));
        if arc == last || i >= 14 {
            break;
        }
    }
    i - 1
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LunarDate {
    pub day: i64,
    pub month: i64,
    pub year: i64,
    pub leap: bool,
    pub jd: i64,
}

pub fn solar_to_lunar(dd: i64, mm: i64, yy: i64) -> LunarDate {
    let day_number = jd_from_date(dd, mm, yy);
    let k = int((day_number as f64 - 2415021.076998695) / 29.530588853);
    let mut month_start = new_moon_day(k + 1);
    if month_start > day_number {
        month_start = new_moon_day(k);
    }
    let mut a11 = lunar_month11(yy);
    let mut b11 = a11;
    let mut lunar_year;
    if a11 >= month_start {
        lunar_year = yy;
        a11 = lunar_month11(yy - 1);
    } else {
        lunar_year = yy + 1;
        b11 = lunar_month11(yy + 1);
    }
    let lunar_day = day_number - month_start + 1;
    let diff = int((month_start - a11) as f64 / 29.0);
    let mut leap = false;
    let mut lunar_month = diff + 11;
    if b11 - a11 > 365 {
        let leap_diff = leap_month_offset(a11);
        if diff >= leap_diff {
            lunar_month = diff + 10;
            if diff == leap_diff {
                leap = true;
            }
        }
    }
    if lunar_month > 12 {
        lunar_month -= 12;
    }
    if lunar_month >= 11 && diff < 4 {
        lunar_year -= 1;
    }
    LunarDate { day: lunar_day, month: lunar_month, year: lunar_year, leap, jd: day_number }
}

/// Gregorian (day, month, year) of a Julian day number.
pub fn jd_to_date(jd: i64) -> (i64, i64, i64) {
    let (b, c) = if jd > 2299160 {
        let a = jd + 32044;
        let b = (4 * a + 3) / 146097;
        (b, a - (b * 146097) / 4)
    } else {
        (0, jd + 32082)
    };
    let d = (4 * c + 3) / 1461;
    let e = c - (1461 * d) / 4;
    let m = (5 * e + 2) / 153;
    let day = e - (153 * m + 2) / 5 + 1;
    let month = m + 3 - 12 * (m / 10);
    let year = b * 100 + d - 4800 + m / 10;
    (day, month, year)
}

/// Solar (day, month, year) of a lunar date; None for a leap month that
/// does not exist that year.
pub fn lunar_to_solar(ld: i64, lm: i64, ly: i64, leap: bool) -> Option<(i64, i64, i64)> {
    let (a11, b11) = if lm < 11 {
        (lunar_month11(ly - 1), lunar_month11(ly))
    } else {
        (lunar_month11(ly), lunar_month11(ly + 1))
    };
    let k = int(0.5 + (a11 as f64 - 2415021.076998695) / 29.530588853);
    let mut off = lm - 11;
    if off < 0 {
        off += 12;
    }
    if b11 - a11 > 365 {
        let leap_off = leap_month_offset(a11);
        let mut leap_month = leap_off - 2;
        if leap_month < 0 {
            leap_month += 12;
        }
        if leap && lm != leap_month {
            return None;
        } else if leap || off >= leap_off {
            off += 1;
        }
    } else if leap {
        return None;
    }
    let month_start = new_moon_day(k + off);
    Some(jd_to_date(month_start + ld - 1))
}

pub fn year_can_chi(year: i64) -> String {
    format!("{} {}", CAN[((year + 6) % 10) as usize], CHI[((year + 8) % 12) as usize])
}

pub fn month_can_chi(l: &LunarDate) -> String {
    let can = ((l.year * 12 + l.month + 3) % 10) as usize;
    let chi = ((l.month + 1) % 12) as usize;
    format!("{} {}{}", CAN[can], CHI[chi], if l.leap { " (nhuận)" } else { "" })
}

pub fn day_can_chi(jd: i64) -> String {
    format!("{} {}", CAN[((jd + 9) % 10) as usize], CHI[((jd + 1) % 12) as usize])
}

/// Solar term (one of 24) in effect on a day.
pub fn tiet_khi(jd: i64) -> &'static str {
    let idx = (sun_longitude_rad(jd as f64 + 0.5 - TZ / 24.0) / PI * 12.0).floor() as usize % 24;
    TIET_KHI[idx]
}

/// Auspicious hours (giờ hoàng đạo) for a day, as chi names.
pub fn gio_hoang_dao(jd: i64) -> Vec<&'static str> {
    const PATTERNS: [&str; 6] = ["110100101100", "001101001011", "110011010010", "101100110100", "001011001101", "010010110011"];
    let chi_of_day = ((jd + 1) % 12) as usize;
    let pattern = PATTERNS[chi_of_day % 6].as_bytes();
    (0..12).filter(|&i| pattern[i] == b'1').map(|i| CHI[i]).collect()
}

pub fn month_name(m: i64) -> &'static str {
    MONTH_NAMES[((m - 1).rem_euclid(12)) as usize]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lunar_to_solar_round_trips() {
        assert_eq!(lunar_to_solar(1, 1, 2025, false), Some((29, 1, 2025)));
        assert_eq!(lunar_to_solar(1, 1, 2026, false), Some((17, 2, 2026)));
        // Tết Đinh Mùi 2027: 6/2/2027.
        assert_eq!(lunar_to_solar(1, 1, 2027, false), Some((6, 2, 2027)));
        // 2025 has a leap 6th month: 1/6 nhuận = 25/7/2025.
        assert_eq!(lunar_to_solar(1, 6, 2025, true), Some((25, 7, 2025)));
        assert_eq!(lunar_to_solar(1, 5, 2025, true), None);
        for (d, m, y) in [(10, 11, 2026), (1, 1, 2030), (31, 12, 2027), (15, 8, 2028)] {
            let l = solar_to_lunar(d, m, y);
            assert_eq!(lunar_to_solar(l.day, l.month, l.year, l.leap), Some((d, m, y)));
        }
    }

    #[test]
    fn known_dates() {
        // Tết Ất Tỵ: 29/01/2025 = 1/1 âm lịch 2025.
        let l = solar_to_lunar(29, 1, 2025);
        assert_eq!((l.day, l.month, l.year, l.leap), (1, 1, 2025, false));
        assert_eq!(year_can_chi(2025), "Ất Tỵ");
        // Tết Bính Ngọ: 17/02/2026.
        let l = solar_to_lunar(17, 2, 2026);
        assert_eq!((l.day, l.month, l.year), (1, 1, 2026));
        // Trung thu 2024: 17/09/2024 = 15/8 âm.
        let l = solar_to_lunar(17, 9, 2024);
        assert_eq!((l.day, l.month), (15, 8));
        // 2023 có tháng 2 nhuận: 22/03/2023 = 1/2 nhuận.
        let l = solar_to_lunar(22, 3, 2023);
        assert_eq!((l.day, l.month, l.leap), (1, 2, true));
    }
}
