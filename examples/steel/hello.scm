;;; Steel Examples
;;; Steel is an embeddable Scheme/Lisp dialect written in Rust

;; ============================================
;; 1. Basic Hello World
;; ============================================
(displayln "Hello, Steel!")

;; ============================================
;; 2. Variables and Bindings
;; ============================================
(define name "World")
(define greeting (string-append "Hello, " name "!"))
(displayln greeting)

;; ============================================
;; 3. Functions
;; ============================================
(define (square x)
  (* x x))

(define (cube x)
  (* x x x))

(displayln (square 5))   ;; => 25
(displayln (cube 3))     ;; => 27

;; ============================================
;; 4. Higher-order functions
;; ============================================
(define (apply-twice f x)
  (f (f x)))

(displayln (apply-twice square 2))   ;; => 16  (square of square of 2)
(displayln (apply-twice add1 5))     ;; => 7

;; ============================================
;; 5. Recursion - Factorial
;; ============================================
(define (factorial n)
  (if (<= n 1)
      1
      (* n (factorial (- n 1)))))

(displayln (factorial 10))  ;; => 3628800

;; ============================================
;; 6. Tail-recursive Fibonacci
;; ============================================
(define (fibonacci n)
  (define (fib-iter a b count)
    (if (= count 0)
        b
        (fib-iter (+ a b) a (- count 1))))
  (fib-iter 1 0 n))

(displayln (fibonacci 10))  ;; => 55
(displayln (fibonacci 20))  ;; => 6765

;; ============================================
;; 7. Lists and List Operations
;; ============================================
(define my-list (list 1 2 3 4 5))

(displayln (car my-list))          ;; => 1
(displayln (cdr my-list))          ;; => (2 3 4 5)
(displayln (length my-list))       ;; => 5
(displayln (reverse my-list))      ;; => (5 4 3 2 1)
(displayln (append my-list (list 6 7 8)))  ;; => (1 2 3 4 5 6 7 8)

;; ============================================
;; 8. Map, Filter, Reduce
;; ============================================
(displayln (map square my-list))             ;; => (1 4 9 16 25)
(displayln (filter (lambda (x) (> x 3)) my-list))  ;; => (4 5)
(displayln (foldl + 0 my-list))              ;; => 15

;; ============================================
;; 9. Let bindings and closures
;; ============================================
(define (make-counter)
  (let ((count 0))
    (lambda ()
      (set! count (+ count 1))
      count)))

(define counter (make-counter))
(displayln (counter))  ;; => 1
(displayln (counter))  ;; => 2
(displayln (counter))  ;; => 3

;; ============================================
;; 10. Pattern matching with cond
;; ============================================
(define (describe-number n)
  (cond
    [(negative? n) "negative"]
    [(zero? n)     "zero"]
    [(even? n)     "positive even"]
    [else          "positive odd"]))

(displayln (describe-number -5))   ;; => "negative"
(displayln (describe-number 0))    ;; => "zero"
(displayln (describe-number 4))    ;; => "positive even"
(displayln (describe-number 7))    ;; => "positive odd"

;; ============================================
;; 11. Structs
;; ============================================
(struct Point (x y))

(define p1 (Point 3 4))
(define p2 (Point 1 2))

(define (point-distance p1 p2)
  (let ([dx (- (Point-x p1) (Point-x p2))]
        [dy (- (Point-y p1) (Point-y p2))])
    (sqrt (+ (* dx dx) (* dy dy)))))

(displayln (point-distance p1 p2))

;; ============================================
;; 12. Hash Maps
;; ============================================
(define my-map (hash "name" "Alice" "age" 30 "lang" "Steel"))

(displayln (hash-ref my-map "name"))  ;; => "Alice"
(displayln (hash-ref my-map "age"))   ;; => 30

(define updated-map (hash-insert my-map "city" "Rustville"))
(displayln (hash-ref updated-map "city"))  ;; => "Rustville"

;; ============================================
;; 13. String operations
;; ============================================
(displayln (string-length "Steel"))         ;; => 5
(displayln (string-upcase "hello steel"))   ;; => "HELLO STEEL"
(displayln (string-contains? "Hello Steel" "Steel"))  ;; => #true
(displayln (to-string 42))                  ;; => "42"

;; ============================================
;; 14. Quicksort
;; ============================================
(define (quicksort lst)
  (if (or (null? lst) (null? (cdr lst)))
      lst
      (let* ([pivot (car lst)]
             [rest  (cdr lst)]
             [less    (filter (lambda (x) (< x pivot)) rest)]
             [greater (filter (lambda (x) (>= x pivot)) rest)])
        (append (quicksort less)
                (list pivot)
                (quicksort greater)))))

(displayln (quicksort (list 3 6 1 8 2 9 4 7 5)))
;; => (1 2 3 4 5 6 7 8 9)

;; ============================================
;; 15. Macros
;; ============================================
(define-syntax when
  (syntax-rules ()
    [(when test body ...)
     (if test (begin body ...) void)]))

(when #true
  (displayln "This runs because condition is true!"))

(define-syntax unless
  (syntax-rules ()
    [(unless test body ...)
     (if test void (begin body ...))]))

(unless #false
  (displayln "This runs because condition is false!"))

;; ============================================
;; 16. Pipe / Threading
;; ============================================
(define result
  (let* ((xs (range 0 10))                    ;; (0 1 2 3 4 5 6 7 8 9)
         (xs (filter even? xs))               ;; (0 2 4 6 8)
         (xs (map (lambda (x) (* x x)) xs))   ;; (0 4 16 36 64)
         (xs (foldl + 0 xs)))                  ;; 120
    xs))

(displayln result)  ;; => 120

(displayln "Done! 🚀")
