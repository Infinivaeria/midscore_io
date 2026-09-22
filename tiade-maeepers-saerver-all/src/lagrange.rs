use std::io;

fn main() {
    println!("Enter a number: ");
    let mut number = String::new();
    io::stdin().read_line(&mut number).expect("Failed to read line");
    let number: i64 = number.trim().parse().expect("Please type a number!");

    match lagrange_four_squares(number) {
        Some(squares) => println!("The number {} can be expressed as the sum of four squares: {}", number, squares),
        None => println!("No combination found to express the number as the sum of four squares."),
    }
}

fn lagrange_four_squares(n: i64) -> Option<String> {
    let mut squares = vec![0; 4];

    // First check if n is a perfect square
    if (n as f64).sqrt().fract() == 0.0 {
        squares[0] = (n as f64).sqrt() as i64;
        return Some(format!("{}²", squares[0]));
    }

    // Check if n can be expressed as the sum of two squares
    for i in 1..=((n as f64).sqrt() as i64) {
        if ((n - i * i) as f64).sqrt().fract() == 0.0 {
            squares[0] = i;
            squares[1] = ((n - i * i) as f64).sqrt() as i64;
            return Some(format!("{}² + {}²", squares[0], squares[1]));
        }
    }

    // Check if n can be expressed as the sum of three squares
    for i in 1..=((n as f64).sqrt() as i64) {
        for j in 1..=((n - i * i) as f64).sqrt() as i64 {
            if ((n - i * i - j * j) as f64).sqrt().fract() == 0.0 {
                squares[0] = i;
                squares[1] = j;
                squares[2] = ((n - i * i - j * j) as f64).sqrt() as i64;
                return Some(format!("{}² + {}² + {}²", squares[0], squares[1], squares[2]));
            }
        }
    }

    // Use the four-square theorem to find the four squares
    for i in 1..=((n as f64).sqrt() as i64) {
        for j in 1..=((n - i * i) as f64).sqrt() as i64 {
            for k in 1..=((n - i * i - j * j) as f64).sqrt() as i64 {
                if ((n - i * i - j * j - k * k) as f64).sqrt().fract() == 0.0 {
                    squares[0] = i;
                    squares[1] = j;
                    squares[2] = k;
                    squares[3] = ((n - i * i - j * j - k * k) as f64).sqrt() as i64;
                    return Some(format!("{}² + {}² + {}² + {}²", squares[0], squares[1], squares[2], squares[3]));
                }
            }
        }
    }

    // If no combination is found, return None
    None
}