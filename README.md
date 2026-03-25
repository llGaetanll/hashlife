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

If you were to build a very simple program to compute Conway's game of life, it
might seem natural to store the cells as a 2D array of boolean values. Cell `(x,
y)` is alive if and only if `cells[x][y]` is `true`. Hashlife does not take this
approach. Instead, Hashlife builds the world from increasingly large square
cells. Start with a 1x1 cell (or a 1 cell), either dead or alive. Put 4 of
these in a 2x2 square and you get a 2 cell. Put 4 of these in a square and you
get a 4 cell, and so on... In Hashlife, the entire world is stored as a QuadTree
of cells `2^k` on a side. To be more specific, a world that is `2^10 = 1024`
cells on a side, can be decomposed as 4 cells `512` on a side. Each of those
further decompose until we get back down to the 1 cell.

In practice, we do not start at the 1 cell but at the 4 cell, as 4
cells require exactly 16 bits of information, and store perfectly inside `u16`s.
We then compose four `u16` "rules" to build a "leaf", or an 8 cell.

## How Hashlife computes future world states

So we understand how the algorithm stores the data, but so far we haven't at all
explained how storing things this way allows us to compute Conway's Game of Life, let
alone doing so efficiently.

We start with our *rules* from earlier, the 4 cells. How can we compute the next state
of this cell? The key observation to make is that, for a 4 cell, we can only
really say anything about its 2x2 center, since the cells on the edges of the 4x4 
depend on their neighbors, whose states we dont know. However the inner 2x2 we know for
sure, we have all the neighbors needed to compute the next state.

Another neat fact is that there's not that many possible 4 cells, only 2^16 in fact!
A fun trick here is that we can compute the next state of all 4 cells. In fact we
can store the result in an array where `arr[i]` is the resulting 2x2 for the 4 cell
represented by the number `i` (recall that we can just store 4 cells as `u16`s).

In general, if we have a $2^k$ cell, we can only speak with certainty about its
$2^{k - 1}$ center after $2^{k - 2}$ iterations.

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
- [x] Implement `setbit` function from hlife (allow populating the world with bit by bit)
- [ ] RLE format support
  - [ ] Serialization
  - [x] Deserialization
- [x] Slight cleanups & refactors
- [ ] Add tests to attempt checking for correctness
- [ ] Improve APIs around `read_rle`
- [ ] Add simple benchmarks
- [ ] Add hashing

## RLE format support checklist
- [ ] Support `LifeHistory` rule & history states
- [x] Golly extended rule format (i.e. `B3/S23:P0,5`)

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
