### Example Result (Still WIP, only `c_analyzer` working for now)
```bash
cargo build --release
./target/releases/header2crate /path/to/your/c/file 
``` 

```
--- Summary 1 ---
Header Path: ../c_files/math_operations.h
Description: Header file containing 2 functions, 0 types, and 1 macros
Number of Functions: 2
Number of Types: 0
Number of Macros: 1
Functions:
  - add: int(int a, int b)
  - multiply: int(int a, int b)
Macros:
  - MATH_OPERATIONS_H: int add(int a, int b); (Parameters: None)

--- Summary 2 ---
Header Path: ../c_files/string_utils.h
Description: Header file containing 1 functions, 0 types, and 1 macros
Number of Functions: 1
Number of Types: 0
Number of Macros: 1
Functions:
  - reverse_string: void(char* str)
Macros:
  - STRING_UTILS_H: void reverse_string(char* str); (Parameters: None)

--- Summary 3 ---
Header Path: ../c_files/main.c
Description: Header file containing 0 functions, 0 types, and 0 macros
Number of Functions: 0
Number of Types: 0
Number of Macros: 0
```
