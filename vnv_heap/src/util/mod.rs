// pub(crate) mod debug;

pub(crate) mod modular_linked_list;

pub(crate) fn padding_needed_for(offset: usize, alignment: usize) -> usize {
    let misalignment = offset % alignment;
    if misalignment > 0 {
        // round up to next multiple of `alignment`
        alignment - misalignment
    } else {
        // already a multiple of `alignment`
        0
    }
}


/// efficient way to calculate: ceil(x / y)
pub(crate) fn ceil_div(x: usize, y: usize) -> usize {
    (x + y - 1) / y
}

#[cfg(test)]
mod test {
    use crate::util::ceil_div;

    #[test]
    fn test_ceil_div() {
        // just test a bunch of different values
        for y in 1..100 {
            for x in 0..y*3 {
                let expected_value = if x % y == 0 {
                    x / y
                } else {
                    (x / y) + 1
                };

                assert_eq!(ceil_div(x, y), expected_value);
            }
        }
    }
}