/// Inverts `x` in the finite field with modulus `modulus`.
/// Assumes that `modulus` is prime, but does not check it.
let inverse = |x, modulus|
    if x <= 0 || x >= modulus {
        if x == 0 {
            std::check::panic("Tried to compute the inverse of zero.")
        } else {
            std::check::panic("Tried to compute the inverse of a negative number or a number outside the field.")
        }
    } else {
        reduce(extended_gcd(x, modulus)[0], modulus)
    };

/// Computes `x + y` modulo the modulus.
let add = |x, y, modulus| reduce(x + y, modulus);

/// Computes `x - y` modulo the modulus.
let sub = |x, y, modulus| reduce(x - y, modulus);

/// Computes `x * y` modulo the modulus.
let mul = |x, y, modulus| reduce(x * y, modulus);

/// Computes `x / y` modulo the modulus.
let div = |x, y, modulus| mul(x, inverse(y, modulus), modulus);

/// Reduces `x` modulo `modulus`, so that it is in the range
/// between `0` and `modulus`. Works on negative `x`.
let reduce = |x, modulus|
    if x < 0 {
        (modulus - ((-x) % modulus)) % modulus
    } else {
        x % modulus
    };

let extended_gcd = |a, b|
    if b == 0 {
        if a == 1 {
            [1, 0]
        } else {
            // a is the gcd, but we do not really want to compute it.
            std::check::panic("Inputs are not co-prime, inverse does not exist.")
        }
    } else {
        // TODO this is written in a complicated way
        // because we do not have tuple destructuring assignment
        (|r| [r[1], r[0] - (a / b) * r[1]])(extended_gcd(b, a % b))
    };