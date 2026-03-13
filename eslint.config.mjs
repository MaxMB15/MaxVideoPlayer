import eslint from "@eslint/js";
import tseslint from "typescript-eslint";
import react from "eslint-plugin-react";
import reactHooks from "eslint-plugin-react-hooks";
import eslintConfigPrettier from "eslint-config-prettier";
import globals from "globals";

export default tseslint.config(
	{ ignores: ["dist/**", "node_modules/**", "**/*.min.js", "libs/**"] },
	eslint.configs.recommended,
	...tseslint.configs.recommended,
	{
		plugins: { react: react, "react-hooks": reactHooks },
		settings: {
			react: { version: "detect" },
		},
		languageOptions: {
			globals: { ...globals.browser },
			parserOptions: {
				ecmaFeatures: { jsx: true },
			},
		},
		rules: {
			...react.configs.recommended.rules,
			...reactHooks.configs.recommended.rules,
			"react/react-in-jsx-scope": "off", // React 17+ new JSX transform
			"func-style": ["error", "expression"],
			"react/no-unescaped-entities": "off",
			"react-hooks/exhaustive-deps": "warn",
			"no-empty": ["error", { allowEmptyCatch: true }],
			"@typescript-eslint/no-empty-object-type": "off",
			"@typescript-eslint/no-unused-expressions": "off",
			"no-constant-binary-expression": "off",
		},
	},
	eslintConfigPrettier
);
