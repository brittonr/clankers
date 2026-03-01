;;; Steel Language Examples
;;; Steel is an embeddable Scheme dialect written in Rust

;; ─────────────────────────────────────────
;; 1. Basic Definitions & Arithmetic
;; ─────────────────────────────────────────

(define pi 3.14159265)
(define tau (* 2 pi))

(define (circle-area radius)
  (* pi radius radius))

(define (circle-circumference radius)
  (* tau radius))

(displayln (circle-area 5))          ;; => 78.5398...
(displayln (circle-circumference 5)) ;; => 31.4159...

;; ─────────────────────────────────────────
;; 2. Pattern Matching with `match`
;; ─────────────────────────────────────────

(define (describe-list lst)
  (match lst
    ['()          "empty list"]
    [(list x)     (string-append "singleton: " (to-string x))]
    [(list x y)   (string-append "pair: " (to-string x) ", " (to-string y))]
    [_            (string-append "list of " (to-string (length lst)) " elements")]))

(displayln (describe-list '()))        ;; => "empty list"
(displayln (describe-list '(42)))      ;; => "singleton: 42"
(displayln (describe-list '(1 2)))     ;; => "pair: 1, 2"
(displayln (describe-list '(1 2 3)))   ;; => "list of 3 elements"

;; ─────────────────────────────────────────
;; 3. Higher-Order Functions
;; ─────────────────────────────────────────

(define (compose f g)
  (lambda (x) (f (g x))))

(define add1 (lambda (x) (+ x 1)))
(define double (lambda (x) (* x 2)))

(define add1-then-double (compose double add1))
(define double-then-add1 (compose add1 double))

(displayln (add1-then-double 5))  ;; => 12  ((5+1)*2)
(displayln (double-then-add1 5))  ;; => 11  ((5*2)+1)

;; ─────────────────────────────────────────
;; 4. Recursive Data Structures
;; ─────────────────────────────────────────

;; Classic: Fibonacci with memoization via hash map
(define fib-cache (hash))

(define (fib n)
  (cond
    [(<= n 1) n]
    [(hash-contains? fib-cache n)
     (hash-ref fib-cache n)]
    [else
     (let ([result (+ (fib (- n 1)) (fib (- n 2)))])
       (set! fib-cache (hash-insert fib-cache n result))
       result)]))

(displayln (map fib (range 0 15)))
;; => (0 1 1 2 3 5 8 13 21 34 55 89 144 233 377)

;; ─────────────────────────────────────────
;; 5. List Processing
;; ─────────────────────────────────────────

(define numbers (range 1 21))

;; Filter, map, fold
(define evens (filter (lambda (x) (= (modulo x 2) 0)) numbers))
(define squared (map (lambda (x) (* x x)) evens))
(define sum-of-squares (foldl + 0 squared))

(displayln evens)           ;; => (2 4 6 8 10 12 14 16 18 20)
(displayln squared)         ;; => (4 16 36 64 100 144 196 256 324 400)
(displayln sum-of-squares)  ;; => 1540

;; ─────────────────────────────────────────
;; 6. Structs
;; ─────────────────────────────────────────

(struct Point (x y))

(define (point-distance p1 p2)
  (let ([dx (- (Point-x p2) (Point-x p1))]
        [dy (- (Point-y p2) (Point-y p1))])
    (sqrt (+ (* dx dx) (* dy dy)))))

(define origin (Point 0 0))
(define p (Point 3 4))

(displayln (point-distance origin p))  ;; => 5.0

;; ─────────────────────────────────────────
;; 7. Threading Macros (Pipeline Style)
;; ─────────────────────────────────────────

;; Steel supports the ~> (thread-first) macro for clean pipelines
(define result
  (~> (range 1 11)
      (filter (lambda (x) (> x 3)))
      (map (lambda (x) (* x x)))
      (foldl + 0)))

(displayln result)  ;; => 330 (sum of squares from 4..10)

;; ─────────────────────────────────────────
;; 8. Error Handling
;; ─────────────────────────────────────────

(define (safe-divide a b)
  (if (= b 0)
      (error "Division by zero!")
      (/ a b)))

(displayln (safe-divide 10 3))  ;; => 3.333...

;; ─────────────────────────────────────────
;; 9. Closures & Encapsulation (Counter)
;; ─────────────────────────────────────────

(define (make-counter start)
  (let ([count start])
    (lambda (msg)
      (match msg
        ['get    count]
        ['inc    (begin (set! count (+ count 1)) count)]
        ['dec    (begin (set! count (- count 1)) count)]
        ['reset  (begin (set! count start) count)]
        [_       (error "Unknown message" msg)]))))

(define counter (make-counter 0))
(displayln (counter 'inc))    ;; => 1
(displayln (counter 'inc))    ;; => 2
(displayln (counter 'inc))    ;; => 3
(displayln (counter 'dec))    ;; => 2
(displayln (counter 'get))    ;; => 2
(displayln (counter 'reset))  ;; => 0

;; ─────────────────────────────────────────
;; 10. Tail-Recursive Iteration
;; ─────────────────────────────────────────

(define (factorial n)
  (define (fact-iter acc n)
    (if (<= n 1)
        acc
        (fact-iter (* acc n) (- n 1))))
  (fact-iter 1 n))

(displayln (factorial 10))   ;; => 3628800
(displayln (factorial 20))   ;; => 2432902008176640000
