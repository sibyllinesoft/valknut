# 💩 Code Smells - Python

This repository contains examples and explanations of common code smells in Python. Code smells are indicators of potential problems or areas of improvement in code quality and design. By recognizing and addressing code smells, developers can create cleaner, more maintainable, and efficient code.

## 📚 Table of Contents

- [Introduction](#-introduction)
- [Code Smells](#-code-smells)
- [How to Explore This Repository](#-how-to-explore-this-repository).
- [Contributing](#-contributing)
- [License](#-license)

## 🧠 Introduction

Code smells are specific patterns or structures in code that indicate potential issues or areas for improvement. They are not bugs or errors but rather indications that the code could be refactored or redesigned for better readability, performance, or maintainability.

This repository focuses on code smells specific to Python programming. Each code smell example includes an explanation of the smell, its potential impact, and suggestions for refactoring or improving the code.

## 💩 Code Smells

The following code smells are covered in this repository:

1. **Magic Numbers**: Using numeric literals without clear meaning or explanation in the code.
2. **Long Method**: A method or function that is excessively long and performs multiple tasks.
3. **Duplicate Code**: Repeated blocks of code that could be extracted into reusable functions or classes.
4. **Large Class**: A class that has grown too large and handles multiple responsibilities.
5. **Feature Envy**: A method that accesses data or behavior from another class excessively.
6. **Inappropriate Intimacy**: Classes that are overly dependent on each other, indicating poor encapsulation.
7. **Data Clumps**: Groups of related data that appear together in multiple places, suggesting they could be encapsulated into a separate class or structure.
8. **Primitive Obsession**: Overuse of primitive data types instead of creating dedicated classes or structures.
9. **Long Parameter List**: Methods or functions that require a large number of parameters, which can make the code harder to understand and maintain.

## 🤯 How to explore this repository
Each code smell will have a dedicated folder containing a code example, an explanation of the smell, and suggestions for refactoring or improving the code. The files that start with `before` contain the original code with code smells, while the files that start with `after` contain the same code but refactored to reduce code smells.

## 👌 Contributing

Contributions to this repository are welcome! If you have additional code smells examples, improvements, or suggestions, feel free to submit a pull request. Please ensure that your contributions align with the existing format and guidelines.

## 🤝 License

This repository is licensed under the [MIT License](LICENSE). You are free to use, modify, and distribute the code examples for both personal and commercial purposes.

Enjoy exploring and learning about code smells in Python!
