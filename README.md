Userful resources:
- [Original Paper by Bill Gosper](https://usr.lmf.cnrs.fr/~jcf/m1/gol/gosper-84.pdf)
- [Wikipedia](https://en.wikipedia.org/wiki/Hashlife)
- [Life Wiki](https://conwaylife.com/wiki/HashLife#cite_note-trokicki20060401-3)
- [Life Lexicon](https://conwaylife.com/ref/lexicon/lex_h.htm#hashlife)
- [Hlife](https://tomas.rokicki.com/hlife/)
- [Dr. Dobb's Journal](http://www.ddj.com/dept/ai/184406478)
    - [Archive Link](https://web.archive.org/web/20120719224016/http://www.drdobbs.com/jvm/an-algorithm-for-compressing-space-and-t/184406478)
- [Hashlife Explained](https://web.archive.org/web/20220131050938/https://jennyhasahat.github.io/hashlife.html)
- [Johnhw Hashlife](https://johnhw.github.io/hashlife/index.md.html)

Notes

- on an `n` cell, if we want to figure out its state in `k` iterations, the largest
knowable cell is an `n - 2k` cell

Optimization Ideas/Questions

*Benchmark, benchmark, benchmark!*

- What's the max number of cells we could ever pack in an array? Knowing such an
  upper bound could allow us to pack more info in 4 words
- SIMD on `256` bit register fits a cell in memory, could make ops faster?
- How likely is it that parallelization would help here?
- Should we use a `HashMap` instead of effectively making our own?
