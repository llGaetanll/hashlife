# `hashlife`

[Hashlife](https://en.wikipedia.org/wiki/Hashlife) is a program that efficiently
computes Conway's Game of Life in large universes. A simple program might take
as input an array of cells, and compute the next state on that array. What makes
Hashlife special is that it allows us to calculate many iterations in the
future, sometimes at no cost at all.

## How does it work?

There are two parts to Hashlife that make it clever. The first is how we store
the cells, and the second is re-using computation. We'll get to caching a bit
later but let's first talk about how the cells are stored.

## How Hashlife stores the cells

If you were to build a very simple prgram to compute Conway's game of life, it
might seem natural to store the cells as a 2D array of boolean values. Cell `(x,
y)` is alive if and only if `cells[x][y]` is `true`. Hashlife does not take this
approach. Instead, Hashlife builds the world from increasingly large square
cells. Start with a 1x1 cell (or a 1 cell), either dead or alive. Put 4 of
these in a 2x2 square and you get a 2 cell. Put 4 of these in a square and you
get a 4 cell, and so on... In Hashlife, the entire world is stored as a QuadTree
of cells `2^k` on a side. To be more specific, a world that is `2^10 = 1024`
cells on a side, can be decomposed as 4 cells `512` on a side. Each of those
further decompose until we get back down to the 1 cell.

In practice, HashLife does not start at the 1 cell but often at the 4 cell, as 4
cells require 16 bits of information, they store perfectly inside `u16`s.

## Re-using Computation

TODO

## What makes Hashlife so efficient

## Notes

- On an `n` cell, if we want to figure out its state in `k` iterations, the largest
  knowable cell is an `n - 2k` cell.
- A 4x4 cell is called a "rule".
- A leaf cell is size 8x8. It's composed of a `u16` rule in all 4 of its quadrants.
- Cells build up from the 8x8 as expected.

## Optimization Ideas/Questions

*Benchmark, benchmark, benchmark!*

- What's the max number of cells we could ever pack in an array? Knowing such an
  upper bound could allow us to pack more info in 4 words
- SIMD on `256` bit register fits a cell in memory, could make ops faster?
- How likely is it that parallelization would help here?
- Should we use a `HashMap` instead of effectively making our own?

## TODO

- [x] Make compute leaf code work
- [x] Draw non-leaf node
- [x] Make compute node code work
- [x] Add function to world to grow it by a factor of 2. The function should
      also keep the old root centered at the origin
- [x] Make drawing worlds easier. The camera should probably take the world and
      be able to draw it (allowing for movement down the line)
      This is fine but now we can't draw the results of `compute_node_res*`.
- [x] Write `compute_node_res` for cells larger than 16
- [x] Make `compute_res` modify the world root. Be sure to grow the resulting
      world back to the size it was before. Avoids the "shrinking world" problem
- [x] Implement drawing the right way
- [x] Implement basic game loop with movements (`ui` example)
- [ ] Implement `setbit` function from hlife (allow populating the world with bit by bit)
- [ ] Implement (de)serialization for RLE format (load and save external patterns)

- [ ] Add tests to attempt checking for correctness
- [ ] Slight cleanups & refactors
- [ ] Add simple benchmarks

- [ ] Add hashing

## Further reading
- [Original Paper by Bill Gosper](https://usr.lmf.cnrs.fr/~jcf/m1/gol/gosper-84.pdf)
- [Wikipedia](https://en.wikipedia.org/wiki/Hashlife)
- [Life Wiki](https://conwaylife.com/wiki/HashLife#cite_note-trokicki20060401-3)
- [Life Lexicon](https://conwaylife.com/ref/lexicon/lex_h.htm#hashlife)
- [Hlife](https://tomas.rokicki.com/hlife/)
- [Dr. Dobb's Journal](http://www.ddj.com/dept/ai/184406478)
    - [Archive Link](https://web.archive.org/web/20120719224016/http://www.drdobbs.com/jvm/an-algorithm-for-compressing-space-and-t/184406478)
- [Hashlife Explained](https://web.archive.org/web/20220131050938/https://jennyhasahat.github.io/hashlife.html)
- [Johnhw Hashlife](https://johnhw.github.io/hashlife/index.md.html)
