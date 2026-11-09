;; The buffers of Peras, shared by every allocator: arrays and strings.
;;
;; Included by build.rs into an allocator module, which provides `$alloc`,
;; `$release`, `$raise`, and the globals `$TOO_LARGE` and `$DEEP`.

  ;; Buffers: the arrays and strings of Peras. A buffer is a 16-byte block, its
  ;; header, which keeps the same address when the buffer grows:
  ;;
  ;;   +0 length   +4 capacity, both counted in elements   +8 elements, or 0
  ;;   +12 the size of an element in bytes, plus DEEP when its elements are
  ;;       buffers themselves, which it owns: an array of strings
  ;;
  ;; The elements are a block of their own, `capacity * size` bytes. Every
  ;; `size` argument below may carry DEEP; only its low byte is a size.

  ;; A new, empty buffer with room for `cap` elements of `size` bytes.
  (func $buf_new (export "buf_new") (param $cap i32) (param $size i32) (param $site i32) (result i32)
    (local $h i32)
    (local.set $h (call $alloc (i32.const 16) (local.get $site)))
    (i32.store (local.get $h) (i32.const 0))
    (i32.store offset=4 (local.get $h) (local.get $cap))
    (i32.store offset=8 (local.get $h)
      (if (result i32) (i32.eqz (local.get $cap))
        (then (i32.const 0))
        (else (call $alloc (call $bytes (local.get $cap) (local.get $size) (local.get $site)) (local.get $site)))))
    (i32.store offset=12 (local.get $h) (local.get $size))
    (local.get $h))

  ;; `count * size`, or a fault when it exceeds what one block can hold.
  (func $bytes (param $count i32) (param $size i32) (param $site i32) (result i32)
    (local $wide i64)
    (local.set $wide (i64.mul (i64.extend_i32_u (local.get $count))
      (i64.extend_i32_u (i32.and (local.get $size) (i32.const 0xFF)))))
    (if (i64.gt_u (local.get $wide) (i64.const 0x40000000))
      (then (call $raise (global.get $TOO_LARGE) (i32.const 0x40000001) (local.get $site) (i32.const 0))))
    (i32.wrap_i64 (local.get $wide)))

  ;; Makes room for `extra` more elements: the capacity at least doubles, so
  ;; adding elements one by one costs a constant time each, on average.
  (func $buf_reserve (export "buf_reserve") (param $h i32) (param $extra i32) (param $size i32) (param $site i32)
    (local $len i32) (local $cap i32) (local $need i64) (local $new_cap i32) (local $data i32)
    (local.set $len (i32.load (local.get $h)))
    (local.set $cap (i32.load offset=4 (local.get $h)))
    (local.set $need (i64.add (i64.extend_i32_u (local.get $len)) (i64.extend_i32_u (local.get $extra))))
    (if (i64.le_u (local.get $need) (i64.extend_i32_u (local.get $cap)))
      (then (return)))
    (if (i64.gt_u (local.get $need) (i64.const 0x40000000))
      (then (call $raise (global.get $TOO_LARGE) (i32.const 0x40000001) (local.get $site) (i32.const 0))))
    (local.set $new_cap (i32.shl (local.get $cap) (i32.const 1)))
    (if (i32.lt_u (local.get $new_cap) (i32.wrap_i64 (local.get $need)))
      (then (local.set $new_cap (i32.wrap_i64 (local.get $need)))))
    (if (i32.lt_u (local.get $new_cap) (i32.const 4))
      (then (local.set $new_cap (i32.const 4))))
    (local.set $data
      (call $alloc (call $bytes (local.get $new_cap) (local.get $size) (local.get $site)) (local.get $site)))
    (if (i32.load offset=8 (local.get $h))
      (then
        (memory.copy (local.get $data) (i32.load offset=8 (local.get $h))
          (call $bytes (local.get $len) (local.get $size) (local.get $site)))
        (call $release (i32.load offset=8 (local.get $h)) (local.get $site))))
    (i32.store offset=4 (local.get $h) (local.get $new_cap))
    (i32.store offset=8 (local.get $h) (local.get $data)))

  ;; Releases a buffer: its header first, which catches a buffer released twice
  ;; before its poisoned contents are read, then the buffers it owns, if its
  ;; elements are buffers, then its elements.
  (func $buf_release (export "buf_release") (param $h i32) (param $site i32)
    (local $data i32) (local $len i32) (local $deep i32) (local $i i32)
    (local.set $data (i32.load offset=8 (local.get $h)))
    (local.set $len (i32.load (local.get $h)))
    (local.set $deep (i32.and (i32.load offset=12 (local.get $h)) (global.get $DEEP)))
    (call $release (local.get $h) (local.get $site))
    (if (local.get $deep)
      (then
        (block $done
          (loop $next
            (br_if $done (i32.ge_u (local.get $i) (local.get $len)))
            (call $buf_release
              (i32.load (i32.add (local.get $data) (i32.shl (local.get $i) (i32.const 2))))
              (local.get $site))
            (local.set $i (i32.add (local.get $i) (i32.const 1)))
            (br $next)))))
    (if (local.get $data)
      (then (call $release (local.get $data) (local.get $site)))))

  ;; A copy of a buffer, with copies of the buffers it owns.
  (func $buf_clone (export "buf_clone") (param $h i32) (param $site i32) (result i32)
    (local $copy i32) (local $len i32) (local $size i32) (local $i i32) (local $at i32)
    (local.set $len (i32.load (local.get $h)))
    (local.set $size (i32.load offset=12 (local.get $h)))
    (local.set $copy (call $buf_new (local.get $len) (local.get $size) (local.get $site)))
    (if (local.get $len)
      (then
        (memory.copy (i32.load offset=8 (local.get $copy)) (i32.load offset=8 (local.get $h))
          (call $bytes (local.get $len) (local.get $size) (local.get $site)))))
    (i32.store (local.get $copy) (local.get $len))
    (if (i32.and (local.get $size) (global.get $DEEP))
      (then
        (block $done
          (loop $next
            (br_if $done (i32.ge_u (local.get $i) (local.get $len)))
            (local.set $at (i32.add (i32.load offset=8 (local.get $copy)) (i32.shl (local.get $i) (i32.const 2))))
            (i32.store (local.get $at) (call $buf_clone (i32.load (local.get $at)) (local.get $site)))
            (local.set $i (i32.add (local.get $i) (i32.const 1)))
            (br $next)))))
    (local.get $copy))

  ;; Strings: buffers of bytes, in UTF-8. Every index and length counts bytes;
  ;; an index that falls inside a character moves back to its start. Each
  ;; function lends its operands and gives a new buffer, owned by the caller.

  ;; A new string of the `len` bytes at `src`.
  (func $str_from (param $src i32) (param $len i32) (result i32)
    (local $h i32)
    (local.set $h (call $buf_new (local.get $len) (i32.const 1) (i32.const 0)))
    (if (local.get $len)
      (then (memory.copy (i32.load offset=8 (local.get $h)) (local.get $src) (local.get $len))))
    (i32.store (local.get $h) (local.get $len))
    (local.get $h))

  ;; Adds the `len` bytes at `src` at the end of the string `h`.
  (func $str_add (param $h i32) (param $src i32) (param $len i32)
    (if (i32.eqz (local.get $len)) (then (return)))
    (call $buf_reserve (local.get $h) (local.get $len) (i32.const 1) (i32.const 0))
    (memory.copy
      (i32.add (i32.load offset=8 (local.get $h)) (i32.load (local.get $h)))
      (local.get $src) (local.get $len))
    (i32.store (local.get $h) (i32.add (i32.load (local.get $h)) (local.get $len))))

  ;; Whether the `n` bytes at `a` and at `b` are the same.
  (func $same (param $a i32) (param $b i32) (param $n i32) (result i32)
    (local $i i32)
    (block $differ
      (loop $next
        (if (i32.ge_u (local.get $i) (local.get $n)) (then (return (i32.const 1))))
        (br_if $differ (i32.ne
          (i32.load8_u (i32.add (local.get $a) (local.get $i)))
          (i32.load8_u (i32.add (local.get $b) (local.get $i)))))
        (local.set $i (i32.add (local.get $i) (i32.const 1)))
        (br $next)))
    (i32.const 0))

  ;; Whether byte `i` of the string `h` continues a character.
  (func $continues (param $h i32) (param $i i32) (result i32)
    (i32.eq
      (i32.and (i32.load8_u (i32.add (i32.load offset=8 (local.get $h)) (local.get $i))) (i32.const 0xC0))
      (i32.const 0x80)))

  ;; An index of `h` made valid: negative counts from the end, then clamped
  ;; to the length, then moved back to the start of its character.
  (func $index (param $h i32) (param $i i32) (result i32)
    (local $len i32)
    (local.set $len (i32.load (local.get $h)))
    (if (i32.lt_s (local.get $i) (i32.const 0))
      (then (local.set $i (i32.add (local.get $i) (local.get $len)))))
    (if (i32.lt_s (local.get $i) (i32.const 0)) (then (local.set $i (i32.const 0))))
    (if (i32.gt_s (local.get $i) (local.get $len)) (then (local.set $i (local.get $len))))
    (block $done
      (loop $back
        (br_if $done (i32.or (i32.eqz (local.get $i)) (i32.ge_u (local.get $i) (local.get $len))))
        (br_if $done (i32.eqz (call $continues (local.get $h) (local.get $i))))
        (local.set $i (i32.sub (local.get $i) (i32.const 1)))
        (br $back)))
    (local.get $i))

  ;; The first index at or after `from` where `needle` is in `h`, or -1.
  (func $str_index_of (export "str_index_of") (param $h i32) (param $needle i32) (param $from i32) (result i32)
    (local $len i32) (local $n i32) (local $i i32)
    (local.set $len (i32.load (local.get $h)))
    (local.set $n (i32.load (local.get $needle)))
    (local.set $i (local.get $from))
    (if (i32.lt_s (local.get $i) (i32.const 0)) (then (local.set $i (i32.const 0))))
    (block $none
      (loop $next
        (br_if $none (i32.gt_u (i32.add (local.get $i) (local.get $n)) (local.get $len)))
        (if (call $same
              (i32.add (i32.load offset=8 (local.get $h)) (local.get $i))
              (i32.load offset=8 (local.get $needle))
              (local.get $n))
          (then (return (local.get $i))))
        (local.set $i (i32.add (local.get $i) (i32.const 1)))
        (br $next)))
    (i32.const -1))

  (func (export "str_starts_with") (param $h i32) (param $prefix i32) (result i32)
    (if (result i32) (i32.gt_u (i32.load (local.get $prefix)) (i32.load (local.get $h)))
      (then (i32.const 0))
      (else (call $same (i32.load offset=8 (local.get $h)) (i32.load offset=8 (local.get $prefix))
        (i32.load (local.get $prefix))))))

  (func (export "str_ends_with") (param $h i32) (param $suffix i32) (result i32)
    (local $len i32) (local $n i32)
    (local.set $len (i32.load (local.get $h)))
    (local.set $n (i32.load (local.get $suffix)))
    (if (result i32) (i32.gt_u (local.get $n) (local.get $len))
      (then (i32.const 0))
      (else (call $same
        (i32.add (i32.load offset=8 (local.get $h)) (i32.sub (local.get $len) (local.get $n)))
        (i32.load offset=8 (local.get $suffix)) (local.get $n)))))

  ;; The bytes from `start` to `end`.
  (func (export "str_slice") (param $h i32) (param $start i32) (param $end i32) (result i32)
    (local.set $start (call $index (local.get $h) (local.get $start)))
    (local.set $end (call $index (local.get $h) (local.get $end)))
    (if (i32.lt_s (local.get $end) (local.get $start)) (then (local.set $end (local.get $start))))
    (call $str_from (i32.add (i32.load offset=8 (local.get $h)) (local.get $start))
      (i32.sub (local.get $end) (local.get $start))))

  ;; Whether a byte is a space, a tab or a line break.
  (func $blank (param $b i32) (result i32)
    (i32.or (i32.or (i32.eq (local.get $b) (i32.const 32)) (i32.eq (local.get $b) (i32.const 9)))
      (i32.or (i32.eq (local.get $b) (i32.const 10)) (i32.eq (local.get $b) (i32.const 13)))))

  (func (export "str_trim") (param $h i32) (result i32)
    (local $data i32) (local $a i32) (local $b i32)
    (local.set $data (i32.load offset=8 (local.get $h)))
    (local.set $b (i32.load (local.get $h)))
    (block $done
      (loop $next
        (br_if $done (i32.ge_u (local.get $a) (local.get $b)))
        (br_if $done (i32.eqz (call $blank (i32.load8_u (i32.add (local.get $data) (local.get $a))))))
        (local.set $a (i32.add (local.get $a) (i32.const 1)))
        (br $next)))
    (block $done
      (loop $next
        (br_if $done (i32.le_u (local.get $b) (local.get $a)))
        (br_if $done (i32.eqz (call $blank
          (i32.load8_u (i32.add (local.get $data) (i32.sub (local.get $b) (i32.const 1)))))))
        (local.set $b (i32.sub (local.get $b) (i32.const 1)))
        (br $next)))
    (call $str_from (i32.add (local.get $data) (local.get $a)) (i32.sub (local.get $b) (local.get $a))))

  ;; A copy with the ASCII letters from `lo` to `lo + 25` shifted by `by`.
  (func $str_case (param $h i32) (param $lo i32) (param $by i32) (result i32)
    (local $copy i32) (local $at i32) (local $end i32) (local $b i32)
    (local.set $copy (call $str_from (i32.load offset=8 (local.get $h)) (i32.load (local.get $h))))
    (local.set $at (i32.load offset=8 (local.get $copy)))
    (local.set $end (i32.add (local.get $at) (i32.load (local.get $copy))))
    (block $done
      (loop $next
        (br_if $done (i32.ge_u (local.get $at) (local.get $end)))
        (local.set $b (i32.load8_u (local.get $at)))
        (if (i32.lt_u (i32.sub (local.get $b) (local.get $lo)) (i32.const 26))
          (then (i32.store8 (local.get $at) (i32.add (local.get $b) (local.get $by)))))
        (local.set $at (i32.add (local.get $at) (i32.const 1)))
        (br $next)))
    (local.get $copy))

  (func (export "str_upper") (param $h i32) (result i32)
    (call $str_case (local.get $h) (i32.const 97) (i32.const -32)))

  (func (export "str_lower") (param $h i32) (result i32)
    (call $str_case (local.get $h) (i32.const 65) (i32.const 32)))

  (func (export "str_repeat") (param $h i32) (param $n i32) (result i32)
    (local $out i32) (local $i i32) (local $len i32)
    (local.set $len (i32.load (local.get $h)))
    (if (i32.lt_s (local.get $n) (i32.const 0)) (then (local.set $n (i32.const 0))))
    ;; `n * len` bytes; more than one block can hold is refused.
    (if (i64.gt_u (i64.mul (i64.extend_i32_u (local.get $n)) (i64.extend_i32_u (local.get $len)))
          (i64.const 0x40000000))
      (then (call $raise (global.get $TOO_LARGE) (i32.const 0x40000001) (i32.const 0) (i32.const 0))))
    (local.set $out (call $buf_new
      (i32.mul (local.get $n) (local.get $len)) (i32.const 1) (i32.const 0)))
    (block $done
      (loop $next
        (br_if $done (i32.ge_u (local.get $i) (local.get $n)))
        (call $str_add (local.get $out) (i32.load offset=8 (local.get $h)) (local.get $len))
        (local.set $i (i32.add (local.get $i) (i32.const 1)))
        (br $next)))
    (local.get $out))

  ;; `h` with `from` replaced by `to`: the first one, or all when `all`. An
  ;; empty `from` replaces nothing.
  (func $str_replace_in (param $h i32) (param $from i32) (param $to i32) (param $all i32) (result i32)
    (local $out i32) (local $i i32) (local $at i32) (local $n i32) (local $data i32)
    (local.set $data (i32.load offset=8 (local.get $h)))
    (local.set $n (i32.load (local.get $from)))
    (local.set $out (call $buf_new (i32.load (local.get $h)) (i32.const 1) (i32.const 0)))
    (block $done
      (loop $next
        (br_if $done (i32.eqz (local.get $n)))
        (local.set $at (call $str_index_of (local.get $h) (local.get $from) (local.get $i)))
        (br_if $done (i32.lt_s (local.get $at) (i32.const 0)))
        (call $str_add (local.get $out) (i32.add (local.get $data) (local.get $i))
          (i32.sub (local.get $at) (local.get $i)))
        (call $str_add (local.get $out) (i32.load offset=8 (local.get $to)) (i32.load (local.get $to)))
        (local.set $i (i32.add (local.get $at) (local.get $n)))
        (br_if $next (local.get $all))))
    (call $str_add (local.get $out) (i32.add (local.get $data) (local.get $i))
      (i32.sub (i32.load (local.get $h)) (local.get $i)))
    (local.get $out))

  (func (export "str_replace") (param $h i32) (param $from i32) (param $to i32) (result i32)
    (call $str_replace_in (local.get $h) (local.get $from) (local.get $to) (i32.const 0)))

  (func (export "str_replace_all") (param $h i32) (param $from i32) (param $to i32) (result i32)
    (call $str_replace_in (local.get $h) (local.get $from) (local.get $to) (i32.const 1)))

  ;; Adds the string `s` at the end of the array of strings `parts`.
  (func $push_part (param $parts i32) (param $s i32)
    (local $len i32)
    (call $buf_reserve (local.get $parts) (i32.const 1) (i32.or (i32.const 4) (global.get $DEEP)) (i32.const 0))
    (local.set $len (i32.load (local.get $parts)))
    (i32.store (i32.add (i32.load offset=8 (local.get $parts)) (i32.shl (local.get $len) (i32.const 2)))
      (local.get $s))
    (i32.store (local.get $parts) (i32.add (local.get $len) (i32.const 1))))

  ;; The parts of `h` between each `separator`; with an empty separator, its
  ;; characters.
  (func (export "str_split") (param $h i32) (param $separator i32) (result i32)
    (local $parts i32) (local $i i32) (local $j i32) (local $at i32) (local $n i32) (local $len i32) (local $data i32)
    (local.set $parts (call $buf_new (i32.const 0) (i32.or (i32.const 4) (global.get $DEEP)) (i32.const 0)))
    (local.set $len (i32.load (local.get $h)))
    (local.set $data (i32.load offset=8 (local.get $h)))
    (local.set $n (i32.load (local.get $separator)))
    (if (i32.eqz (local.get $n))
      (then
        (block $done
          (loop $next
            (br_if $done (i32.ge_u (local.get $i) (local.get $len)))
            (local.set $j (i32.add (local.get $i) (i32.const 1)))
            (block $end
              (loop $more
                (br_if $end (i32.ge_u (local.get $j) (local.get $len)))
                (br_if $end (i32.eqz (call $continues (local.get $h) (local.get $j))))
                (local.set $j (i32.add (local.get $j) (i32.const 1)))
                (br $more)))
            (call $push_part (local.get $parts)
              (call $str_from (i32.add (local.get $data) (local.get $i)) (i32.sub (local.get $j) (local.get $i))))
            (local.set $i (local.get $j))
            (br $next)))
        (return (local.get $parts))))
    (block $done
      (loop $next
        (local.set $at (call $str_index_of (local.get $h) (local.get $separator) (local.get $i)))
        (br_if $done (i32.lt_s (local.get $at) (i32.const 0)))
        (call $push_part (local.get $parts)
          (call $str_from (i32.add (local.get $data) (local.get $i)) (i32.sub (local.get $at) (local.get $i))))
        (local.set $i (i32.add (local.get $at) (local.get $n)))
        (br $next)))
    (call $push_part (local.get $parts)
      (call $str_from (i32.add (local.get $data) (local.get $i)) (i32.sub (local.get $len) (local.get $i))))
    (local.get $parts))

  ;; The strings of an array, with `separator` between each two.
  (func (export "arr_join") (param $parts i32) (param $separator i32) (result i32)
    (local $out i32) (local $i i32) (local $s i32)
    (local.set $out (call $buf_new (i32.const 0) (i32.const 1) (i32.const 0)))
    (block $done
      (loop $next
        (br_if $done (i32.ge_u (local.get $i) (i32.load (local.get $parts))))
        (if (local.get $i)
          (then (call $str_add (local.get $out) (i32.load offset=8 (local.get $separator))
            (i32.load (local.get $separator)))))
        (local.set $s (i32.load (i32.add (i32.load offset=8 (local.get $parts)) (i32.shl (local.get $i) (i32.const 2)))))
        (call $str_add (local.get $out) (i32.load offset=8 (local.get $s)) (i32.load (local.get $s)))
        (local.set $i (i32.add (local.get $i) (i32.const 1)))
        (br $next)))
    (local.get $out))

  ;; Arrays, by column: a buffer of elements of `size` bytes, each column of
  ;; an array of structures on its own. A column of buffers (DEEP) owns them:
  ;; a new column gets copies.

  ;; The size of an element of `h`, in bytes.
  (func $element_size (param $h i32) (result i32)
    (i32.and (i32.load offset=12 (local.get $h)) (i32.const 0xFF)))

  ;; Each buffer of a new DEEP column replaced by a copy of it.
  (func $own_copies (param $h i32)
    (local $i i32) (local $at i32)
    (if (i32.eqz (i32.and (i32.load offset=12 (local.get $h)) (global.get $DEEP))) (then (return)))
    (block $done
      (loop $next
        (br_if $done (i32.ge_u (local.get $i) (i32.load (local.get $h))))
        (local.set $at (i32.add (i32.load offset=8 (local.get $h)) (i32.shl (local.get $i) (i32.const 2))))
        (i32.store (local.get $at) (call $buf_clone (i32.load (local.get $at)) (i32.const 0)))
        (local.set $i (i32.add (local.get $i) (i32.const 1)))
        (br $next))))

  ;; An index of an array made valid: negative from the end, then clamped.
  (func $clamp_index (param $len i32) (param $i i32) (result i32)
    (if (i32.lt_s (local.get $i) (i32.const 0))
      (then (local.set $i (i32.add (local.get $i) (local.get $len)))))
    (if (i32.lt_s (local.get $i) (i32.const 0)) (then (local.set $i (i32.const 0))))
    (if (i32.gt_s (local.get $i) (local.get $len)) (then (local.set $i (local.get $len))))
    (local.get $i))

  ;; A new column with the elements from `start` to `end`.
  (func (export "arr_slice") (param $h i32) (param $start i32) (param $end i32) (result i32)
    (local $len i32) (local $es i32) (local $n i32) (local $out i32)
    (local.set $len (i32.load (local.get $h)))
    (local.set $es (call $element_size (local.get $h)))
    (local.set $start (call $clamp_index (local.get $len) (local.get $start)))
    (local.set $end (call $clamp_index (local.get $len) (local.get $end)))
    (if (i32.lt_s (local.get $end) (local.get $start)) (then (local.set $end (local.get $start))))
    (local.set $n (i32.sub (local.get $end) (local.get $start)))
    (local.set $out (call $buf_new (local.get $n) (i32.load offset=12 (local.get $h)) (i32.const 0)))
    (if (local.get $n)
      (then (memory.copy (i32.load offset=8 (local.get $out))
        (i32.add (i32.load offset=8 (local.get $h)) (i32.mul (local.get $start) (local.get $es)))
        (i32.mul (local.get $n) (local.get $es)))))
    (i32.store (local.get $out) (local.get $n))
    (call $own_copies (local.get $out))
    (local.get $out))

  ;; A new column with the elements of `a`, then those of `b`.
  (func (export "arr_concat") (param $a i32) (param $b i32) (result i32)
    (local $la i32) (local $lb i32) (local $es i32) (local $out i32)
    (local.set $la (i32.load (local.get $a)))
    (local.set $lb (i32.load (local.get $b)))
    (local.set $es (call $element_size (local.get $a)))
    (local.set $out (call $buf_new (i32.add (local.get $la) (local.get $lb))
      (i32.load offset=12 (local.get $a)) (i32.const 0)))
    (if (local.get $la)
      (then (memory.copy (i32.load offset=8 (local.get $out)) (i32.load offset=8 (local.get $a))
        (i32.mul (local.get $la) (local.get $es)))))
    (if (local.get $lb)
      (then (memory.copy
        (i32.add (i32.load offset=8 (local.get $out)) (i32.mul (local.get $la) (local.get $es)))
        (i32.load offset=8 (local.get $b)) (i32.mul (local.get $lb) (local.get $es)))))
    (i32.store (local.get $out) (i32.add (local.get $la) (local.get $lb)))
    (call $own_copies (local.get $out))
    (local.get $out))

  ;; Swaps the elements at `i` and `j` of `h`, of `es` bytes: 1, 4 or 8.
  (func $swap (param $h i32) (param $i i32) (param $j i32) (param $es i32)
    (local $a i32) (local $b i32) (local $t i64)
    (local.set $a (i32.add (i32.load offset=8 (local.get $h)) (i32.mul (local.get $i) (local.get $es))))
    (local.set $b (i32.add (i32.load offset=8 (local.get $h)) (i32.mul (local.get $j) (local.get $es))))
    (if (i32.eq (local.get $es) (i32.const 8))
      (then
        (local.set $t (i64.load (local.get $a)))
        (i64.store (local.get $a) (i64.load (local.get $b)))
        (i64.store (local.get $b) (local.get $t))
        (return)))
    (if (i32.eq (local.get $es) (i32.const 4))
      (then
        (local.set $t (i64.extend_i32_u (i32.load (local.get $a))))
        (i32.store (local.get $a) (i32.load (local.get $b)))
        (i32.store (local.get $b) (i32.wrap_i64 (local.get $t)))
        (return)))
    (local.set $t (i64.extend_i32_u (i32.load8_u (local.get $a))))
    (i32.store8 (local.get $a) (i32.load8_u (local.get $b)))
    (i32.store8 (local.get $b) (i32.wrap_i64 (local.get $t))))

  ;; Reverses the elements of `h` in place.
  (func (export "arr_reverse") (param $h i32)
    (local $i i32) (local $j i32) (local $es i32)
    (local.set $es (call $element_size (local.get $h)))
    (local.set $j (i32.sub (i32.load (local.get $h)) (i32.const 1)))
    (block $done
      (loop $next
        (br_if $done (i32.ge_s (local.get $i) (local.get $j)))
        (call $swap (local.get $h) (local.get $i) (local.get $j) (local.get $es))
        (local.set $i (i32.add (local.get $i) (i32.const 1)))
        (local.set $j (i32.sub (local.get $j) (i32.const 1)))
        (br $next))))

  ;; Whether element `i` of `h` is below element `j`, as numbers of `kind`:
  ;; 0 `i32`, 1 `i64`, 2 `f32`, 3 `f64`.
  (func $below (param $h i32) (param $i i32) (param $j i32) (param $kind i32) (result i32)
    (local $data i32)
    (local.set $data (i32.load offset=8 (local.get $h)))
    (if (i32.eqz (local.get $kind))
      (then (return (i32.lt_s
        (i32.load (i32.add (local.get $data) (i32.shl (local.get $i) (i32.const 2))))
        (i32.load (i32.add (local.get $data) (i32.shl (local.get $j) (i32.const 2))))))))
    (if (i32.eq (local.get $kind) (i32.const 1))
      (then (return (i64.lt_s
        (i64.load (i32.add (local.get $data) (i32.shl (local.get $i) (i32.const 3))))
        (i64.load (i32.add (local.get $data) (i32.shl (local.get $j) (i32.const 3))))))))
    (if (i32.eq (local.get $kind) (i32.const 2))
      (then (return (f32.lt
        (f32.load (i32.add (local.get $data) (i32.shl (local.get $i) (i32.const 2))))
        (f32.load (i32.add (local.get $data) (i32.shl (local.get $j) (i32.const 2))))))))
    (f64.lt
      (f64.load (i32.add (local.get $data) (i32.shl (local.get $i) (i32.const 3))))
      (f64.load (i32.add (local.get $data) (i32.shl (local.get $j) (i32.const 3))))))

  ;; Moves the element at `root` down the heap of the first `end` elements.
  (func $sift (param $h i32) (param $root i32) (param $end i32) (param $kind i32) (param $es i32)
    (local $child i32)
    (block $done
      (loop $next
        (local.set $child (i32.add (i32.shl (local.get $root) (i32.const 1)) (i32.const 1)))
        (br_if $done (i32.ge_s (local.get $child) (local.get $end)))
        (if (i32.lt_s (i32.add (local.get $child) (i32.const 1)) (local.get $end))
          (then (if (call $below (local.get $h) (local.get $child) (i32.add (local.get $child) (i32.const 1)) (local.get $kind))
            (then (local.set $child (i32.add (local.get $child) (i32.const 1)))))))
        (br_if $done (i32.eqz (call $below (local.get $h) (local.get $root) (local.get $child) (local.get $kind))))
        (call $swap (local.get $h) (local.get $root) (local.get $child) (local.get $es))
        (local.set $root (local.get $child))
        (br $next))))

  ;; Sorts the numbers of `h` in increasing order, in place: heapsort, in
  ;; time n log n, without allocating.
  (func (export "arr_sort") (param $h i32) (param $kind i32)
    (local $n i32) (local $i i32) (local $es i32)
    (local.set $n (i32.load (local.get $h)))
    (local.set $es (call $element_size (local.get $h)))
    (local.set $i (i32.sub (i32.shr_u (local.get $n) (i32.const 1)) (i32.const 1)))
    (block $built
      (loop $next
        (br_if $built (i32.lt_s (local.get $i) (i32.const 0)))
        (call $sift (local.get $h) (local.get $i) (local.get $n) (local.get $kind) (local.get $es))
        (local.set $i (i32.sub (local.get $i) (i32.const 1)))
        (br $next)))
    (local.set $i (i32.sub (local.get $n) (i32.const 1)))
    (block $sorted
      (loop $next
        (br_if $sorted (i32.le_s (local.get $i) (i32.const 0)))
        (call $swap (local.get $h) (i32.const 0) (local.get $i) (local.get $es))
        (call $sift (local.get $h) (i32.const 0) (local.get $i) (local.get $kind) (local.get $es))
        (local.set $i (i32.sub (local.get $i) (i32.const 1)))
        (br $next))))

  ;; The index of the first string of `h` equal to `s`, or -1.
  (func (export "arr_index_of_str") (param $h i32) (param $s i32) (result i32)
    (local $i i32) (local $e i32)
    (block $none
      (loop $next
        (br_if $none (i32.ge_u (local.get $i) (i32.load (local.get $h))))
        (local.set $e (i32.load (i32.add (i32.load offset=8 (local.get $h)) (i32.shl (local.get $i) (i32.const 2)))))
        (if (i32.eq (i32.load (local.get $e)) (i32.load (local.get $s)))
          (then (if (call $same (i32.load offset=8 (local.get $e)) (i32.load offset=8 (local.get $s)) (i32.load (local.get $s)))
            (then (return (local.get $i))))))
        (local.set $i (i32.add (local.get $i) (i32.const 1)))
        (br $next)))
    (i32.const -1))
