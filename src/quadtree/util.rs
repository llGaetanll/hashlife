/// Takes `&mut [T]` and a predicate `P: FnMut(&T) -> bool` and partitions the list according to
/// the predicate. Swaps elements of the list such that all elements satisfying `P` appear before
/// any element not satisfying `P`.
///
/// The index `i` returned by the function always points at the first element for which `P` is false.
/// Note that if `P` is trivial, then `i = |list|` points outside the list.
pub fn partition_in_place<T, P>(list: &mut [T], mut predicate: P) -> usize
where
    P: FnMut(&T) -> bool,
{
    if list.is_empty() {
        return 0;
    }

    let (mut lo, mut hi) = (0, list.len() - 1);

    while lo < hi {
        if predicate(&list[lo]) {
            lo += 1;
            continue;
        }

        if !predicate(&list[hi]) {
            hi -= 1;
            continue;
        }

        list.swap(lo, hi);
        lo += 1;
        hi -= 1;
    }

    if predicate(&list[lo]) {
        lo + 1
    } else {
        lo
    }
}

#[cfg(test)]
mod test {
    use super::partition_in_place;

    #[test]
    fn partition_simple() {
        let mut lst = [3, 6, 7, 8, 5, 2, 9, 4, 1, 10];

        let i = partition_in_place(&mut lst, |x| *x < 5);

        assert_eq!(lst, [3, 1, 4, 2, 5, 8, 9, 7, 6, 10]);
        assert_eq!(i, 4);
    }

    #[test]
    fn partition_sorted() {
        let mut lst = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10];

        let i = partition_in_place(&mut lst, |x| *x < 5);

        assert_eq!(lst, [1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);
        assert_eq!(i, 4);
    }

    #[test]
    fn partition_trivial_pred() {
        let mut lst = [3, 6, 7, 8, 5, 2, 9, 4, 1, 10];

        let i = partition_in_place(&mut lst, |x| *x < 11);

        assert_eq!(lst, [3, 6, 7, 8, 5, 2, 9, 4, 1, 10]);
        assert_eq!(i, 10);
    }

    #[test]
    fn partition_bools() {
        let lsts = [
            (vec![], vec![], 0),
            (vec![0], vec![0], 0),
            (vec![1], vec![1], 1),
            (vec![0, 0], vec![0, 0], 0),
            (vec![0, 1], vec![1, 0], 1),
            (vec![1, 0], vec![1, 0], 1),
            (vec![1, 1], vec![1, 1], 2),
            (vec![0, 1, 1], vec![1, 1, 0], 2),
            (vec![0, 1, 1, 1], vec![1, 1, 1, 0], 3),
            (vec![0, 1, 1, 1, 1, 1], vec![1, 1, 1, 1, 1, 0], 5),
            (vec![0, 1, 0, 0, 0, 1], vec![1, 1, 0, 0, 0, 0], 2),
        ];

        for (mut have, want, i) in lsts {
            let j = partition_in_place(&mut have, |n| *n == 1);

            assert_eq!(have, want);
            assert_eq!(j, i, "expected {} but got {} for list {:?}", i, j, want);
        }
    }
}
