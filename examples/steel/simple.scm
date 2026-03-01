;;; simple.scm — A simple Steel script
;;; A tiny todo-list manager using closures

;; Create a new todo list (closure over a mutable list)
(define (make-todo-list)
  (let ((items '()))
    (lambda (action . args)
      (cond
        [(equal? action 'add)
         (set! items (append items (list (car args))))
         (displayln (string-append "Added: " (car args)))]
        [(equal? action 'done)
         (let ((task (car args)))
           (set! items (filter (lambda (i) (not (equal? i task))) items))
           (displayln (string-append "Done: " task)))]
        [(equal? action 'list)
         (if (null? items)
             (displayln "No tasks! 🎉")
             (begin
               (displayln "--- Todo List ---")
               (for-each
                 (lambda (item)
                   (displayln (string-append "  • " item)))
                 items)
               (displayln (string-append "  (" (int->string (length items)) " tasks)"))))]
        [else (displayln "Unknown action. Use 'add, 'done, or 'list.")]))))

;; --- Demo ---
(displayln "=== Steel Todo List ===\n")

(define my-todos (make-todo-list))

(my-todos 'add "Learn Steel")
(my-todos 'add "Write some Scheme")
(my-todos 'add "Build something cool")
(my-todos 'add "Take a break")

(displayln "")
(my-todos 'list)

(displayln "")
(my-todos 'done "Write some Scheme")
(my-todos 'done "Take a break")

(displayln "")
(my-todos 'list)
