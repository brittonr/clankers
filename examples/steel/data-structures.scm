;;; Steel Data Structures & Algorithms
;;; A collection of functional data structures implemented in Steel

;; ═══════════════════════════════════════════
;; 1. Binary Search Tree (BST)
;; ═══════════════════════════════════════════

(struct BST (value left right))

(define bst-empty 'nil)

(define (bst-empty? tree)
  (equal? tree 'nil))

(define (bst-insert tree val)
  (cond
    [(bst-empty? tree) (BST val bst-empty bst-empty)]
    [(< val (BST-value tree))
     (BST (BST-value tree)
          (bst-insert (BST-left tree) val)
          (BST-right tree))]
    [(> val (BST-value tree))
     (BST (BST-value tree)
          (BST-left tree)
          (bst-insert (BST-right tree) val))]
    [else tree])) ;; duplicate, ignore

(define (bst-contains? tree val)
  (cond
    [(bst-empty? tree) #false]
    [(= val (BST-value tree)) #true]
    [(< val (BST-value tree)) (bst-contains? (BST-left tree) val)]
    [else (bst-contains? (BST-right tree) val)]))

(define (bst-inorder tree)
  (if (bst-empty? tree)
      '()
      (append (bst-inorder (BST-left tree))
              (list (BST-value tree))
              (bst-inorder (BST-right tree)))))

(define (bst-min tree)
  (cond
    [(bst-empty? tree) (error "empty tree")]
    [(bst-empty? (BST-left tree)) (BST-value tree)]
    [else (bst-min (BST-left tree))]))

(define (bst-max tree)
  (cond
    [(bst-empty? tree) (error "empty tree")]
    [(bst-empty? (BST-right tree)) (BST-value tree)]
    [else (bst-max (BST-right tree))]))

(define (bst-depth tree)
  (if (bst-empty? tree)
      0
      (+ 1 (max (bst-depth (BST-left tree))
                 (bst-depth (BST-right tree))))))

(define (bst-size tree)
  (if (bst-empty? tree)
      0
      (+ 1 (bst-size (BST-left tree))
             (bst-size (BST-right tree)))))

;; Build a BST from a list
(define (bst-from-list lst)
  (foldl (lambda (val tree) (bst-insert tree val)) bst-empty lst))

;; Demo: BST
(displayln "═══ Binary Search Tree ═══")
(define my-tree (bst-from-list (list 5 3 7 1 4 6 8 2 9)))
(displayln (~> my-tree bst-inorder))        ;; => (1 2 3 4 5 6 7 8 9)
(displayln (bst-contains? my-tree 4))       ;; => #true
(displayln (bst-contains? my-tree 10))      ;; => #false
(displayln (string-append "min: " (to-string (bst-min my-tree))))  ;; => min: 1
(displayln (string-append "max: " (to-string (bst-max my-tree))))  ;; => max: 9
(displayln (string-append "depth: " (to-string (bst-depth my-tree))))
(displayln (string-append "size: " (to-string (bst-size my-tree))))

;; ═══════════════════════════════════════════
;; 2. Functional Stack (LIFO)
;; ═══════════════════════════════════════════

(struct Stack (items count))

(define (stack-new) (Stack '() 0))

(define (stack-push stk val)
  (Stack (cons val (Stack-items stk))
         (+ 1 (Stack-count stk))))

(define (stack-pop stk)
  (if (null? (Stack-items stk))
      (error "Cannot pop empty stack")
      (Stack (cdr (Stack-items stk))
             (- (Stack-count stk) 1))))

(define (stack-peek stk)
  (if (null? (Stack-items stk))
      (error "Cannot peek empty stack")
      (car (Stack-items stk))))

(define (stack-empty? stk)
  (null? (Stack-items stk)))

(define (stack-size stk) (Stack-count stk))

(define (stack->list stk) (Stack-items stk))

;; Demo: Stack
(displayln "")
(displayln "═══ Functional Stack ═══")
(define s (~> (stack-new)
              (stack-push 10)
              (stack-push 20)
              (stack-push 30)))
(displayln (stack->list s))       ;; => (30 20 10)
(displayln (stack-peek s))        ;; => 30
(displayln (stack-size s))        ;; => 3
(define s2 (stack-pop s))
(displayln (stack-peek s2))       ;; => 20

;; ═══════════════════════════════════════════
;; 3. Functional Queue (FIFO) - two-stack impl
;; ═══════════════════════════════════════════

(struct Queue (inbox outbox))

(define (queue-new) (Queue '() '()))

(define (queue-enqueue q val)
  (Queue (cons val (Queue-inbox q))
         (Queue-outbox q)))

(define (queue--normalize q)
  (if (null? (Queue-outbox q))
      (Queue '() (reverse (Queue-inbox q)))
      q))

(define (queue-dequeue q)
  (let ([nq (queue--normalize q)])
    (if (null? (Queue-outbox nq))
        (error "Cannot dequeue empty queue")
        (Queue (Queue-inbox nq)
               (cdr (Queue-outbox nq))))))

(define (queue-front q)
  (let ([nq (queue--normalize q)])
    (if (null? (Queue-outbox nq))
        (error "Cannot peek empty queue")
        (car (Queue-outbox nq)))))

(define (queue-empty? q)
  (and (null? (Queue-inbox q))
       (null? (Queue-outbox q))))

(define (queue->list q)
  (let ([nq (queue--normalize q)])
    (append (Queue-outbox nq) (reverse (Queue-inbox nq)))))

;; Demo: Queue
(displayln "")
(displayln "═══ Functional Queue ═══")
(define q (~> (queue-new)
              (queue-enqueue "first")
              (queue-enqueue "second")
              (queue-enqueue "third")))
(displayln (queue->list q))     ;; => ("first" "second" "third")
(displayln (queue-front q))     ;; => "first"
(define q2 (queue-dequeue q))
(displayln (queue-front q2))    ;; => "second"
(displayln (queue->list q2))    ;; => ("second" "third")

;; ═══════════════════════════════════════════
;; 4. Sorting Algorithms
;; ═══════════════════════════════════════════

;; Merge Sort
(define (merge-sort lst)
  (define (merge xs ys)
    (cond
      [(null? xs) ys]
      [(null? ys) xs]
      [(<= (car xs) (car ys))
       (cons (car xs) (merge (cdr xs) ys))]
      [else
       (cons (car ys) (merge xs (cdr ys)))]))
  (define (split lst)
    (define (go slow fast)
      (if (or (null? fast) (null? (cdr fast)))
          slow
          (go (cdr slow) (cdr (cdr fast)))))
    (go lst lst))
  (define (take-left lst mid)
    (if (equal? lst mid)
        '()
        (cons (car lst) (take-left (cdr lst) mid))))
  (if (or (null? lst) (null? (cdr lst)))
      lst
      (let* ([mid (split lst)]
             [left (take-left lst mid)]
             [right mid])
        (merge (merge-sort left) (merge-sort right)))))

;; Tree Sort (uses BST above)
(define (tree-sort lst)
  (bst-inorder (bst-from-list lst)))

;; Demo: Sorting
(displayln "")
(displayln "═══ Sorting Algorithms ═══")
(define unsorted (list 38 27 43 3 9 82 10))
(displayln (string-append "unsorted:    " (to-string unsorted)))
(displayln (string-append "merge-sort:  " (to-string (merge-sort unsorted))))
(displayln (string-append "tree-sort:   " (to-string (tree-sort unsorted))))

;; ═══════════════════════════════════════════
;; 5. Graph (adjacency list) + BFS
;; ═══════════════════════════════════════════

;; Graph is represented as a hash: node -> list-of-neighbors
(define (graph-new) (hash))

(define (graph-add-edge g from to)
  (let ([neighbors (if (hash-contains? g from)
                       (hash-ref g from)
                       '())])
    (hash-insert g from (cons to neighbors))))

(define (graph-neighbors g node)
  (if (hash-contains? g node)
      (hash-ref g node)
      '()))

;; BFS returning the path of visited nodes
(define (graph-bfs g start)
  (define (bfs-iter queue visited result)
    (if (queue-empty? queue)
        (reverse result)
        (let* ([node (queue-front queue)]
               [queue (queue-dequeue queue)])
          (if (hash-contains? visited node)
              (bfs-iter queue visited result)
              (let* ([visited (hash-insert visited node #true)]
                     [result (cons node result)]
                     [neighbors (graph-neighbors g node)]
                     [queue (foldl (lambda (n q)
                                     (if (hash-contains? visited n)
                                         q
                                         (queue-enqueue q n)))
                                   queue
                                   neighbors)])
                (bfs-iter queue visited result))))))
  (bfs-iter (queue-enqueue (queue-new) start)
            (hash)
            '()))

;; Demo: Graph BFS
(displayln "")
(displayln "═══ Graph BFS ═══")
(define g (~> (graph-new)
              (graph-add-edge 'A 'B)
              (graph-add-edge 'A 'C)
              (graph-add-edge 'B 'D)
              (graph-add-edge 'B 'E)
              (graph-add-edge 'C 'F)
              (graph-add-edge 'D 'G)
              (graph-add-edge 'E 'G)
              (graph-add-edge 'F 'G)))

(displayln (string-append "BFS from A: " (to-string (graph-bfs g 'A))))
;; Visits: A -> B, C -> D, E, F -> G

;; ═══════════════════════════════════════════
;; 6. Sieve of Eratosthenes
;; ═══════════════════════════════════════════

(define (sieve limit)
  (define (mark-multiples nums p)
    (filter (lambda (n) (or (= n p) (not (= 0 (modulo n p))))) nums))
  (define (sieve-iter nums primes)
    (if (null? nums)
        (reverse primes)
        (let ([p (car nums)])
          (if (> (* p p) limit)
              (append (reverse primes) nums)
              (sieve-iter (mark-multiples (cdr nums) p)
                          (cons p primes))))))
  (sieve-iter (range 2 (+ limit 1)) '()))

(displayln "")
(displayln "═══ Sieve of Eratosthenes ═══")
(displayln (string-append "Primes up to 50: " (to-string (sieve 50))))
(displayln (string-append "Count of primes up to 100: " (to-string (length (sieve 100)))))

;; ═══════════════════════════════════════════
;; 7. Functional State Machine
;; ═══════════════════════════════════════════

(struct FSM (state transitions))

(define (fsm-new initial-state transitions)
  (FSM initial-state transitions))

(define (fsm-transition machine event)
  (let* ([state (FSM-state machine)]
         [key (list state event)]
         [transitions (FSM-transitions machine)])
    (if (hash-contains? transitions key)
        (FSM (hash-ref transitions key) transitions)
        (error (string-append "No transition from "
                              (to-string state) " on event " (to-string event))))))

(define (fsm-state-of machine) (FSM-state machine))

(define (fsm-run machine events)
  (foldl (lambda (event m) (fsm-transition m event))
         machine
         events))

;; Demo: Traffic Light FSM
(displayln "")
(displayln "═══ Finite State Machine ═══")
(define traffic-light
  (fsm-new 'red
    (hash (list 'red 'next)    'green
          (list 'green 'next)  'yellow
          (list 'yellow 'next) 'red)))

(define light-after-3
  (fsm-run traffic-light (list 'next 'next 'next)))

(displayln (string-append "Start: red"))
(displayln (string-append "After 1 next: "
  (to-string (fsm-state-of (fsm-run traffic-light (list 'next))))))
(displayln (string-append "After 2 next: "
  (to-string (fsm-state-of (fsm-run traffic-light (list 'next 'next))))))
(displayln (string-append "After 3 next: "
  (to-string (fsm-state-of light-after-3))))

(displayln "")
(displayln "All done! 🎉")
