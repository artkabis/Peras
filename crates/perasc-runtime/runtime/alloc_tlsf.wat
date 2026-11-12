;; The production allocator of owned Peras modules: TLSF, Two-Level
;; Segregated Fit (Masmano, Ripoll, Crespo and Real, 2004).
;;
;; Every operation takes a bounded time, whatever the size requested and the
;; state of the heap: no loop depends on the number of blocks.
;;
;; - Free blocks are kept in 384 lists, by size class: a first level per power
;;   of two, split into 16 second-level classes. Two bitmaps say which lists
;;   are not empty; finding a block that fits is two `i32.ctz`, and the class
;;   of a size is one `i32.clz`.
;; - A block larger than needed is split, its remainder kept free.
;; - A released block merges at once with the free blocks just before and
;;   after it, so two free blocks are never neighbours; one that ends the heap
;;   gives its memory back to it.
;;
;; It checks only what costs nothing: a pointer that is not one of its blocks,
;; and a block released twice. The debug allocator catches the rest, in
;; development.
;;
;; Memory layout:
;;
;;   0 .. 1024       never used: address 0 is never a valid pointer
;;   1024 .. 1120    second-level bitmaps, one i32 per first level
;;   2048 .. 3584    free-list heads, 24 first levels * 16 classes
;;   3584 .. 3648    heads of the blocks kept for reuse, one per size of 32 to 256
;;   3648 .. 3712    how many each holds
;;   4096 ..         the heap, growing upward
;;
;; A block is a 16-byte header, then the user's bytes; the pointer given to
;; the user is 16-byte aligned:
;;
;;   +0 size of the whole block, a multiple of 16; bit 0 set when it is free
;;   +4 the block just before it in memory, or 0 for the first one
;;   +8 next and +12 previous free block of its list, when it is free
(module
  (import "peras" "fault" (func $fault (param i32 i32 i32 i32)))
  ;; A float as text, written at `at` by the host, which gives its length:
  ;; JavaScript's `String()`, or for an `f32` (`single`), the fewest digits
  ;; that read back as it. At most 25 ASCII bytes.
  (import "peras" "float_text" (func $float_text (param f64 i32 i32) (result i32)))
  ;; The number the `len` bytes at `at` write, by JavaScript's `Number`: what
  ;; `str_parse_float` cannot read exactly by itself.
  (import "peras" "float_parse" (func $float_parse (param i32 i32) (result f64)))

  (memory (export "memory") 1)

  ;; End of the heap: the first byte no block uses.
  (global $top (mut i32) (i32.const 4096))
  ;; The last block of the heap, which is never free, or 0.
  (global $last (mut i32) (i32.const 0))
  ;; Which first levels have a free block.
  (global $fl_bitmap (mut i32) (i32.const 0))
  (global $serial (mut i32) (i32.const 0))
  (global $live_count (mut i32) (i32.const 0))
  (global $live_bytes (mut i32) (i32.const 0))

  ;; A small block released is kept for the next allocation of its size, the
  ;; last kept the first given (decision 0016): a released block of 32 to 256
  ;; bytes goes to the list of its size, at most 64 per size, marked KEPT. It
  ;; is not free, so its neighbours do not merge with it; released again, it
  ;; is caught. Allocating and releasing a small block then costs a few loads
  ;; and stores, instead of a search of the free lists and a merge. `trim`
  ;; gives the kept blocks back.
  (global $KEPT i32 (i32.const 2))
  (global $KEEP_MAX i32 (i32.const 64))

  ;; Fault kinds (see `Fault` in lib.rs).
  (global $OUT_OF_MEMORY i32 (i32.const 1))
  (global $TOO_LARGE i32 (i32.const 2))
  (global $INVALID_RELEASE i32 (i32.const 4))
  (global $DOUBLE_RELEASE i32 (i32.const 5))
  (global $LEAK i32 (i32.const 7))

  ;; In a buffer's element size: its elements are buffers it owns.
  (global $DEEP i32 (i32.const 0x100))

  (func $raise (param $kind i32) (param $ptr i32) (param $alloc_site i32) (param $free_site i32)
    (call $fault (local.get $kind) (local.get $ptr) (local.get $alloc_site) (local.get $free_site))
    unreachable)

  ;; Makes the memory at least `end` bytes long, or faults.
  (func $reserve (param $start i32) (param $end i32) (param $site i32)
    (local $needed i64)
    (if (i32.lt_u (local.get $end) (local.get $start))
      (then (call $raise (global.get $OUT_OF_MEMORY) (i32.const 0) (local.get $site) (i32.const 0))))
    (local.set $needed
      (i64.shr_u (i64.add (i64.extend_i32_u (local.get $end)) (i64.const 65535)) (i64.const 16)))
    (if (i64.gt_u (local.get $needed) (i64.extend_i32_u (memory.size)))
      (then
        (if (i32.eq
              (memory.grow (i32.wrap_i64 (i64.sub (local.get $needed) (i64.extend_i32_u (memory.size)))))
              (i32.const -1))
          (then (call $raise (global.get $OUT_OF_MEMORY) (i32.const 0) (local.get $site) (i32.const 0)))))))

  ;; The size of a block, without its free bit.
  (func $size (param $block i32) (result i32)
    (i32.and (i32.load (local.get $block)) (i32.const -16)))

  (func $is_free (param $block i32) (result i32)
    (i32.and (i32.load (local.get $block)) (i32.const 1)))

  ;; The list of blocks of `size` bytes: first level * 16 + second level.
  ;; Below 256 bytes, one class per 16 bytes; above, 16 classes per power of 2.
  (func $class (param $size i32) (result i32)
    (local $f i32)
    (if (i32.lt_u (local.get $size) (i32.const 256))
      (then (return (i32.shr_u (local.get $size) (i32.const 4)))))
    ;; The position of the highest bit, 8 to 30.
    (local.set $f (i32.sub (i32.const 31) (i32.clz (local.get $size))))
    (i32.add
      (i32.shl (i32.sub (local.get $f) (i32.const 7)) (i32.const 4))
      (i32.and (i32.shr_u (local.get $size) (i32.sub (local.get $f) (i32.const 4))) (i32.const 15))))

  ;; Where the head of a list is.
  (func $head (param $class i32) (result i32)
    (i32.add (i32.const 2048) (i32.shl (local.get $class) (i32.const 2))))

  ;; Where the second-level bitmap of a first level is.
  (func $sl_bitmap (param $fl i32) (result i32)
    (i32.add (i32.const 1024) (i32.shl (local.get $fl) (i32.const 2))))

  ;; Adds a free block of `size` bytes to its list.
  (func $insert (param $block i32) (param $size i32)
    (local $class i32) (local $next i32) (local $fl i32)
    (local.set $class (call $class (local.get $size)))
    (local.set $fl (i32.shr_u (local.get $class) (i32.const 4)))
    (local.set $next (i32.load (call $head (local.get $class))))
    (i32.store (local.get $block) (i32.or (local.get $size) (i32.const 1)))
    (i32.store offset=8 (local.get $block) (local.get $next))
    (i32.store offset=12 (local.get $block) (i32.const 0))
    (if (local.get $next)
      (then (i32.store offset=12 (local.get $next) (local.get $block))))
    (i32.store (call $head (local.get $class)) (local.get $block))
    (i32.store (call $sl_bitmap (local.get $fl))
      (i32.or (i32.load (call $sl_bitmap (local.get $fl)))
        (i32.shl (i32.const 1) (i32.and (local.get $class) (i32.const 15)))))
    (global.set $fl_bitmap
      (i32.or (global.get $fl_bitmap) (i32.shl (i32.const 1) (local.get $fl)))))

  ;; Takes a free block out of its list.
  (func $remove (param $block i32)
    (local $class i32) (local $next i32) (local $prev i32) (local $fl i32) (local $map i32)
    (local.set $class (call $class (call $size (local.get $block))))
    (local.set $fl (i32.shr_u (local.get $class) (i32.const 4)))
    (local.set $next (i32.load offset=8 (local.get $block)))
    (local.set $prev (i32.load offset=12 (local.get $block)))
    (if (local.get $next)
      (then (i32.store offset=12 (local.get $next) (local.get $prev))))
    (if (local.get $prev)
      (then (i32.store offset=8 (local.get $prev) (local.get $next)))
      (else
        (i32.store (call $head (local.get $class)) (local.get $next))
        ;; The list is empty now: so say the bitmaps.
        (if (i32.eqz (local.get $next))
          (then
            (local.set $map
              (i32.and (i32.load (call $sl_bitmap (local.get $fl)))
                (i32.xor (i32.shl (i32.const 1) (i32.and (local.get $class) (i32.const 15))) (i32.const -1))))
            (i32.store (call $sl_bitmap (local.get $fl)) (local.get $map))
            (if (i32.eqz (local.get $map))
              (then (global.set $fl_bitmap
                (i32.and (global.get $fl_bitmap)
                  (i32.xor (i32.shl (i32.const 1) (local.get $fl)) (i32.const -1))))))))))
    ;; Used again: the free bit goes.
    (i32.store (local.get $block) (call $size (local.get $block))))

  ;; A free block of at least `size` bytes, taken out of its list, or 0.
  (func $find (param $size i32) (result i32)
    (local $class i32) (local $fl i32) (local $map i32) (local $f i32) (local $block i32)
    ;; Rounded up to the next class: any block of that class fits.
    (if (i32.ge_u (local.get $size) (i32.const 256))
      (then
        (local.set $f (i32.sub (i32.const 31) (i32.clz (local.get $size))))
        (local.set $size (i32.add (local.get $size)
          (i32.sub (i32.shl (i32.const 1) (i32.sub (local.get $f) (i32.const 4))) (i32.const 1))))))
    (local.set $class (call $class (local.get $size)))
    (local.set $fl (i32.shr_u (local.get $class) (i32.const 4)))
    ;; Classes of this first level from this one up, then the next levels.
    (local.set $map (i32.and (i32.load (call $sl_bitmap (local.get $fl)))
      (i32.shl (i32.const -1) (i32.and (local.get $class) (i32.const 15)))))
    (if (i32.eqz (local.get $map))
      (then
        (local.set $map (i32.and (global.get $fl_bitmap)
          (i32.shl (i32.const -1) (i32.add (local.get $fl) (i32.const 1)))))
        (if (i32.eqz (local.get $map))
          (then (return (i32.const 0))))
        (local.set $fl (i32.ctz (local.get $map)))
        (local.set $map (i32.load (call $sl_bitmap (local.get $fl))))))
    (local.set $block (i32.load (call $head
      (i32.add (i32.shl (local.get $fl) (i32.const 4)) (i32.ctz (local.get $map))))))
    (call $remove (local.get $block))
    (local.get $block))

  ;; Allocates `size` bytes; the pointer is 16-byte aligned.
  (func $alloc (export "alloc") (param $size i32) (param $site i32) (result i32)
    (local $need i32) (local $block i32) (local $have i32) (local $rest i32) (local $kept i32)
    (if (i32.gt_u (local.get $size) (i32.const 0x40000000))
      (then (call $raise (global.get $TOO_LARGE) (local.get $size) (local.get $site) (i32.const 0))))
    ;; The user's bytes rounded up to 16, and the header; 32 bytes at least,
    ;; so that a free block holds its links.
    (local.set $need (i32.add (i32.and (i32.add (local.get $size) (i32.const 15)) (i32.const -16)) (i32.const 16)))
    (if (i32.lt_u (local.get $need) (i32.const 32))
      (then (local.set $need (i32.const 32))))
    ;; A block of this size kept from a release: given at once.
    (if (i32.le_u (local.get $need) (i32.const 256))
      (then
        (local.set $kept (i32.add (i32.const 3584)
          (i32.shl (i32.sub (i32.shr_u (local.get $need) (i32.const 4)) (i32.const 2)) (i32.const 2))))
        (local.set $block (i32.load (local.get $kept)))
        (if (local.get $block)
          (then
            (i32.store (local.get $kept) (i32.load offset=8 (local.get $block)))
            (i32.store offset=64 (local.get $kept)
              (i32.sub (i32.load offset=64 (local.get $kept)) (i32.const 1)))
            (i32.store (local.get $block) (local.get $need))
            (global.set $serial (i32.add (global.get $serial) (i32.const 1)))
            (global.set $live_count (i32.add (global.get $live_count) (i32.const 1)))
            (global.set $live_bytes (i32.add (global.get $live_bytes)
              (i32.sub (local.get $need) (i32.const 16))))
            (return (i32.add (local.get $block) (i32.const 16)))))))
    (local.set $block (call $find (local.get $need)))
    (if (local.get $block)
      (then
        (local.set $have (call $size (local.get $block)))
        ;; What is left, if it can be a block, stays free.
        (if (i32.ge_u (i32.sub (local.get $have) (local.get $need)) (i32.const 32))
          (then
            (local.set $rest (i32.add (local.get $block) (local.get $need)))
            (i32.store offset=4 (local.get $rest) (local.get $block))
            ;; A free block never ends the heap: something follows it.
            (i32.store offset=4 (i32.add (local.get $block) (local.get $have)) (local.get $rest))
            (call $insert (local.get $rest) (i32.sub (local.get $have) (local.get $need)))
            (i32.store (local.get $block) (local.get $need)))))
      (else
        ;; No free block fits: the heap grows.
        (local.set $block (global.get $top))
        (call $reserve (local.get $block) (i32.add (local.get $block) (local.get $need)) (local.get $site))
        (i32.store (local.get $block) (local.get $need))
        (i32.store offset=4 (local.get $block) (global.get $last))
        (global.set $last (local.get $block))
        (global.set $top (i32.add (local.get $block) (local.get $need)))))
    (global.set $serial (i32.add (global.get $serial) (i32.const 1)))
    (global.set $live_count (i32.add (global.get $live_count) (i32.const 1)))
    (global.set $live_bytes (i32.add (global.get $live_bytes)
      (i32.sub (call $size (local.get $block)) (i32.const 16))))
    (i32.add (local.get $block) (i32.const 16)))

  ;; Releases the block at `ptr`, merged at once with its free neighbours.
  (func $release (export "release") (param $ptr i32) (param $site i32)
    (local $block i32) (local $size i32) (local $kept i32)
    (if (i32.or (i32.or
          (i32.lt_u (local.get $ptr) (i32.const 4112))
          (i32.ge_u (local.get $ptr) (global.get $top)))
          (i32.and (local.get $ptr) (i32.const 15)))
      (then (call $raise (global.get $INVALID_RELEASE) (local.get $ptr) (i32.const 0) (local.get $site))))
    (local.set $block (i32.sub (local.get $ptr) (i32.const 16)))
    (if (i32.and (i32.load (local.get $block)) (i32.or (i32.const 1) (global.get $KEPT)))
      (then (call $raise (global.get $DOUBLE_RELEASE) (local.get $ptr) (i32.const 0) (local.get $site))))
    (local.set $size (call $size (local.get $block)))
    (if (i32.or (i32.lt_u (local.get $size) (i32.const 32))
          (i32.gt_u (i32.add (local.get $block) (local.get $size)) (global.get $top)))
      (then (call $raise (global.get $INVALID_RELEASE) (local.get $ptr) (i32.const 0) (local.get $site))))
    (global.set $live_count (i32.sub (global.get $live_count) (i32.const 1)))
    (global.set $live_bytes (i32.sub (global.get $live_bytes) (i32.sub (local.get $size) (i32.const 16))))
    ;; Small: kept for the next allocation of its size, while there is room.
    (if (i32.le_u (local.get $size) (i32.const 256))
      (then
        (local.set $kept (i32.add (i32.const 3584)
          (i32.shl (i32.sub (i32.shr_u (local.get $size) (i32.const 4)) (i32.const 2)) (i32.const 2))))
        (if (i32.lt_u (i32.load offset=64 (local.get $kept)) (global.get $KEEP_MAX))
          (then
            (i32.store (local.get $block) (i32.or (local.get $size) (global.get $KEPT)))
            (i32.store offset=8 (local.get $block) (i32.load (local.get $kept)))
            (i32.store (local.get $kept) (local.get $block))
            (i32.store offset=64 (local.get $kept)
              (i32.add (i32.load offset=64 (local.get $kept)) (i32.const 1)))
            (return)))))
    (call $give_back (local.get $block) (local.get $size)))

  ;; The block at `block`, of `size` bytes, made free, merged at once with its
  ;; free neighbours.
  (func $give_back (param $block i32) (param $size i32)
    (local $next i32) (local $prev i32)
    ;; The free block after it.
    (local.set $next (i32.add (local.get $block) (local.get $size)))
    (if (i32.lt_u (local.get $next) (global.get $top))
      (then
        (if (call $is_free (local.get $next))
          (then
            (call $remove (local.get $next))
            (local.set $size (i32.add (local.get $size) (call $size (local.get $next))))))))
    ;; The free block before it.
    (local.set $prev (i32.load offset=4 (local.get $block)))
    (if (local.get $prev)
      (then
        (if (call $is_free (local.get $prev))
          (then
            (call $remove (local.get $prev))
            (local.set $size (i32.add (local.get $size) (call $size (local.get $prev))))
            (local.set $block (local.get $prev))))))
    ;; At the end of the heap: the heap shrinks back.
    (if (i32.eq (i32.add (local.get $block) (local.get $size)) (global.get $top))
      (then
        (global.set $top (local.get $block))
        (global.set $last (i32.load offset=4 (local.get $block)))
        (return)))
    (i32.store offset=4 (i32.add (local.get $block) (local.get $size)) (local.get $block))
    (call $insert (local.get $block) (local.get $size)))

  ;; Gives the blocks kept for reuse back to the heap, which shrinks to what
  ;; is alive: what a release did before blocks were kept.
  (func (export "trim")
    (local $kept i32) (local $block i32)
    (local.set $kept (i32.const 3584))
    (block $done
      (loop $class
        (br_if $done (i32.ge_u (local.get $kept) (i32.const 3644)))
        (block $empty
          (loop $next
            (local.set $block (i32.load (local.get $kept)))
            (br_if $empty (i32.eqz (local.get $block)))
            (i32.store (local.get $kept) (i32.load offset=8 (local.get $block)))
            (call $give_back (local.get $block) (call $size (local.get $block)))
            (br $next)))
        (i32.store offset=64 (local.get $kept) (i32.const 0))
        (local.set $kept (i32.add (local.get $kept) (i32.const 4)))
        (br $class))))

  ;; @include buffers.wat

  (func (export "live_count") (result i32) (global.get $live_count))
  (func (export "live_bytes") (result i32) (global.get $live_bytes))
  (func (export "allocations") (result i32) (global.get $serial))
  ;; The end of the heap, for measurements.
  (func (export "heap_end") (result i32) (global.get $top))

  ;; What the debug allocator records per block is not kept here.
  (func (export "next_live") (param $after i32) (result i32) (i32.const 0))
  (func (export "block_size") (param $ptr i32) (result i32)
    (i32.sub (call $size (i32.sub (local.get $ptr) (i32.const 16))) (i32.const 16)))
  (func (export "block_site") (param $ptr i32) (result i32) (i32.const 0))
  (func (export "block_serial") (param $ptr i32) (result i32) (i32.const 0))

  ;; At the end of a program: faults if a block is still alive.
  (func (export "check_leaks")
    (if (global.get $live_count)
      (then (call $raise (global.get $LEAK) (i32.const 0) (i32.const 0) (global.get $live_count)))))
)
