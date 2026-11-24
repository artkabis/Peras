;; The debug allocator of owned Peras modules (decision 0001, precondition 2).
;;
;; Slow on purpose: it trades speed for catching every memory bug of the code
;; perasc generates, as soon as it happens.
;;
;; - Released memory is poisoned (0xDD) and kept in quarantine before it can be
;;   reused; a write into it is caught when it leaves quarantine or is reused.
;; - Every block is followed by a red zone (0xFD), checked on release: a write
;;   one byte past the end is caught.
;; - Fresh memory is filled with 0xCD, so a read of an uninitialized byte shows.
;; - A double release, or the release of anything that is not a live block,
;;   is caught before it touches memory.
;; - `check_leaks` reports the first block still alive, with the site that
;;   allocated it; `next_live` lists them all.
;;
;; A fault calls the host's `peras.fault(kind, pointer, alloc_site, free_site)`,
;; which throws a readable error, then traps. Kinds are listed in
;; `perasc-runtime/src/lib.rs`. Sites are numbers perasc gives to each place in
;; the source that allocates or releases; 0 means unknown.
;;
;; Memory layout:
;;
;;   0 .. 1024       never used: address 0 is never a valid pointer
;;   1024 .. 2052    free-list heads, one per size class (16, 32, ... 4096 bytes)
;;   2052            free-list head for larger blocks
;;   2304 .. 3328    quarantine: a ring of 256 freed blocks
;;   4096 ..         the heap, growing upward
;;
;; A block is a 32-byte header, the user's bytes rounded up to 16, and a 16-byte
;; red zone. The pointer given to the user is 16-byte aligned:
;;
;;   +0 magic   +4 size   +8 alloc site   +12 serial   +16 free site   +20 next
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

  ;; First free byte of the heap.
  (global $top (mut i32) (i32.const 4096))
  ;; Allocations made so far; also the serial of the last one.
  (global $serial (mut i32) (i32.const 0))
  (global $live_count (mut i32) (i32.const 0))
  (global $live_bytes (mut i32) (i32.const 0))
  ;; Quarantine ring: index of the oldest entry, and number of entries.
  (global $q_head (mut i32) (i32.const 0))
  (global $q_len (mut i32) (i32.const 0))

  ;; Header magic numbers.
  (global $LIVE i32 (i32.const 0xA110CA7E))
  (global $QUARANTINED i32 (i32.const 0xF7EE0001))
  (global $REUSABLE i32 (i32.const 0xF7EE0002))

  ;; Fault kinds (see `Fault` in lib.rs).
  (global $OUT_OF_MEMORY i32 (i32.const 1))
  (global $TOO_LARGE i32 (i32.const 2))
  (global $OVERFLOW i32 (i32.const 3))
  (global $INVALID_RELEASE i32 (i32.const 4))
  (global $DOUBLE_RELEASE i32 (i32.const 5))
  (global $USE_AFTER_RELEASE i32 (i32.const 6))
  (global $LEAK i32 (i32.const 7))

  ;; In a buffer's element size: its elements are buffers it owns.
  (global $DEEP i32 (i32.const 0x100))

  ;; Reports a fault to the host, then traps if the host did not throw.
  (func $raise (param $kind i32) (param $ptr i32) (param $alloc_site i32) (param $free_site i32)
    (call $fault (local.get $kind) (local.get $ptr) (local.get $alloc_site) (local.get $free_site))
    unreachable)

  ;; Rounds a size up to a multiple of 16.
  (func $round (param $size i32) (result i32)
    (i32.and (i32.add (local.get $size) (i32.const 15)) (i32.const -16)))

  ;; Bytes a block takes in the heap, header and red zone included.
  (func $span (param $block i32) (result i32)
    (i32.add (call $round (i32.load offset=4 (local.get $block))) (i32.const 48)))

  ;; The free-list head for blocks of `rounded` user bytes.
  (func $head (param $rounded i32) (result i32)
    (if (result i32) (i32.le_u (local.get $rounded) (i32.const 4096))
      (then (i32.add (i32.const 1024) (i32.shl (i32.shr_u (local.get $rounded) (i32.const 4)) (i32.const 2))))
      (else (i32.const 2052))))

  ;; Whether `len` bytes from `start` all equal `byte`.
  (func $all (param $start i32) (param $len i32) (param $byte i32) (result i32)
    (local $end i32)
    (local.set $end (i32.add (local.get $start) (local.get $len)))
    (block $done
      (loop $next
        (br_if $done (i32.ge_u (local.get $start) (local.get $end)))
        (if (i32.ne (i32.load8_u (local.get $start)) (local.get $byte))
          (then (return (i32.const 0))))
        (local.set $start (i32.add (local.get $start) (i32.const 1)))
        (br $next)))
    (i32.const 1))

  ;; Makes the memory at least `end` bytes long, or faults.
  (func $reserve (param $start i32) (param $end i32) (param $site i32)
    (local $needed i64)
    ;; The end wrapped around: more than 4 GiB.
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

  ;; A released block is untouched: its bytes still hold the poison, and its red
  ;; zone the red-zone pattern. Faults otherwise.
  (func $check_untouched (param $block i32)
    (local $rounded i32)
    (local.set $rounded (call $round (i32.load offset=4 (local.get $block))))
    (if (i32.eqz (i32.and
          (call $all (i32.add (local.get $block) (i32.const 32)) (local.get $rounded) (i32.const 0xDD))
          (call $all (i32.add (i32.add (local.get $block) (i32.const 32)) (local.get $rounded)) (i32.const 16) (i32.const 0xFD))))
      (then (call $raise (global.get $USE_AFTER_RELEASE)
        (i32.add (local.get $block) (i32.const 32))
        (i32.load offset=8 (local.get $block))
        (i32.load offset=16 (local.get $block))))))

  ;; Takes a reusable block of exactly `rounded` user bytes, or returns 0.
  (func $take (param $rounded i32) (result i32)
    (local $link i32) (local $block i32)
    (local.set $link (call $head (local.get $rounded)))
    (block $none
      (loop $next
        (local.set $block (i32.load (local.get $link)))
        (br_if $none (i32.eqz (local.get $block)))
        (if (i32.eq (call $round (i32.load offset=4 (local.get $block))) (local.get $rounded))
          (then
            (i32.store (local.get $link) (i32.load offset=20 (local.get $block)))
            (call $check_untouched (local.get $block))
            (return (local.get $block))))
        (local.set $link (i32.add (local.get $block) (i32.const 20)))
        (br $next)))
    (i32.const 0))

  ;; Moves the oldest quarantined block to its free list, once checked.
  (func $evict
    (local $block i32)
    (local.set $block (i32.load (i32.add (i32.const 2304) (i32.shl (global.get $q_head) (i32.const 2)))))
    (global.set $q_head (i32.and (i32.add (global.get $q_head) (i32.const 1)) (i32.const 255)))
    (global.set $q_len (i32.sub (global.get $q_len) (i32.const 1)))
    (call $check_untouched (local.get $block))
    (i32.store (local.get $block) (global.get $REUSABLE))
    (i32.store offset=20 (local.get $block)
      (i32.load (call $head (call $round (i32.load offset=4 (local.get $block))))))
    (i32.store (call $head (call $round (i32.load offset=4 (local.get $block)))) (local.get $block)))

  ;; Allocates `size` bytes for the source site `site`; the pointer is 16-byte aligned.
  (func $alloc (export "alloc") (param $size i32) (param $site i32) (result i32)
    (local $rounded i32) (local $block i32) (local $ptr i32) (local $end i32)
    (if (i32.gt_u (local.get $size) (i32.const 0x40000000))
      (then (call $raise (global.get $TOO_LARGE) (local.get $size) (local.get $site) (i32.const 0))))
    (local.set $rounded (call $round (local.get $size)))
    (local.set $block (call $take (local.get $rounded)))
    (if (i32.eqz (local.get $block))
      (then
        (local.set $block (global.get $top))
        (local.set $end (i32.add (local.get $block) (i32.add (local.get $rounded) (i32.const 48))))
        (call $reserve (local.get $block) (local.get $end) (local.get $site))
        (global.set $top (local.get $end))))
    (global.set $serial (i32.add (global.get $serial) (i32.const 1)))
    (i32.store (local.get $block) (global.get $LIVE))
    (i32.store offset=4 (local.get $block) (local.get $size))
    (i32.store offset=8 (local.get $block) (local.get $site))
    (i32.store offset=12 (local.get $block) (global.get $serial))
    (i32.store offset=16 (local.get $block) (i32.const 0))
    (i32.store offset=20 (local.get $block) (i32.const 0))
    (local.set $ptr (i32.add (local.get $block) (i32.const 32)))
    (memory.fill (local.get $ptr) (i32.const 0xCD) (local.get $size))
    (memory.fill (i32.add (local.get $ptr) (local.get $size)) (i32.const 0xFD)
      (i32.sub (i32.add (local.get $rounded) (i32.const 16)) (local.get $size)))
    (global.set $live_count (i32.add (global.get $live_count) (i32.const 1)))
    (global.set $live_bytes (i32.add (global.get $live_bytes) (local.get $size)))
    (local.get $ptr))

  ;; Releases the block at `ptr`, from the source site `site`.
  (func $release (export "release") (param $ptr i32) (param $site i32)
    (local $block i32) (local $magic i32) (local $size i32) (local $rounded i32)
    (if (i32.or (i32.or
          (i32.lt_u (local.get $ptr) (i32.const 4128))
          (i32.ge_u (local.get $ptr) (global.get $top)))
          (i32.and (local.get $ptr) (i32.const 15)))
      (then (call $raise (global.get $INVALID_RELEASE) (local.get $ptr) (i32.const 0) (local.get $site))))
    (local.set $block (i32.sub (local.get $ptr) (i32.const 32)))
    (local.set $magic (i32.load (local.get $block)))
    (if (i32.or (i32.eq (local.get $magic) (global.get $QUARANTINED)) (i32.eq (local.get $magic) (global.get $REUSABLE)))
      (then (call $raise (global.get $DOUBLE_RELEASE) (local.get $ptr)
        (i32.load offset=8 (local.get $block)) (local.get $site))))
    (if (i32.ne (local.get $magic) (global.get $LIVE))
      (then (call $raise (global.get $INVALID_RELEASE) (local.get $ptr) (i32.const 0) (local.get $site))))
    (local.set $size (i32.load offset=4 (local.get $block)))
    (local.set $rounded (call $round (local.get $size)))
    (if (i32.eqz (call $all (i32.add (local.get $ptr) (local.get $size)) (i32.sub (i32.add (local.get $rounded) (i32.const 16)) (local.get $size)) (i32.const 0xFD)))
      (then (call $raise (global.get $OVERFLOW) (local.get $ptr)
        (i32.load offset=8 (local.get $block)) (local.get $site))))
    ;; Poison the user's bytes; the red zone keeps its pattern.
    (memory.fill (local.get $ptr) (i32.const 0xDD) (local.get $rounded))
    (i32.store (local.get $block) (global.get $QUARANTINED))
    (i32.store offset=16 (local.get $block) (local.get $site))
    (global.set $live_count (i32.sub (global.get $live_count) (i32.const 1)))
    (global.set $live_bytes (i32.sub (global.get $live_bytes) (local.get $size)))
    (if (i32.eq (global.get $q_len) (i32.const 256)) (then (call $evict)))
    (i32.store
      (i32.add (i32.const 2304)
        (i32.shl (i32.and (i32.add (global.get $q_head) (global.get $q_len)) (i32.const 255)) (i32.const 2)))
      (local.get $block))
    (global.set $q_len (i32.add (global.get $q_len) (i32.const 1))))

  ;; The buffers, shared with the production allocator.
  ;; @include buffers.wat

  ;; Nothing is kept for reuse here: a block released waits in quarantine,
  ;; where a write into it is caught. The TLSF allocator's `trim` gives back
  ;; what it keeps; here there is nothing to give.
  (func (export "trim"))

  (func (export "live_count") (result i32) (global.get $live_count))
  (func (export "live_bytes") (result i32) (global.get $live_bytes))
  ;; Allocations made since the start.
  (func (export "allocations") (result i32) (global.get $serial))

  ;; The first live block after `after` (0: from the start), or 0.
  (func $next_live (export "next_live") (param $after i32) (result i32)
    (local $block i32)
    (local.set $block
      (if (result i32) (i32.eqz (local.get $after))
        (then (i32.const 4096))
        (else (i32.add (i32.sub (local.get $after) (i32.const 32))
          (call $span (i32.sub (local.get $after) (i32.const 32)))))))
    (block $none
      (loop $next
        (br_if $none (i32.ge_u (local.get $block) (global.get $top)))
        (if (i32.eq (i32.load (local.get $block)) (global.get $LIVE))
          (then (return (i32.add (local.get $block) (i32.const 32)))))
        (local.set $block (i32.add (local.get $block) (call $span (local.get $block))))
        (br $next)))
    (i32.const 0))

  ;; The size, allocation site and serial of a live block.
  (func (export "block_size") (param $ptr i32) (result i32)
    (i32.load offset=4 (i32.sub (local.get $ptr) (i32.const 32))))
  (func (export "block_site") (param $ptr i32) (result i32)
    (i32.load offset=8 (i32.sub (local.get $ptr) (i32.const 32))))
  (func (export "block_serial") (param $ptr i32) (result i32)
    (i32.load offset=12 (i32.sub (local.get $ptr) (i32.const 32))))

  ;; At the end of a program: faults with the first block still alive, if any.
  (func (export "check_leaks")
    (local $ptr i32)
    (local.set $ptr (call $next_live (i32.const 0)))
    (if (local.get $ptr)
      (then (call $raise (global.get $LEAK) (local.get $ptr)
        (i32.load offset=8 (i32.sub (local.get $ptr) (i32.const 32)))
        (global.get $live_count)))))
)
