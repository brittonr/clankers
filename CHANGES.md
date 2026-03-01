# Changes Made: Nix Integration into Agent System

## Summary
Added Nix ephemeral package usage instructions directly into the default agent system prompt, so all agents automatically know how to handle missing commands/packages without permanently installing them.

## Files Modified

### 1. `src/agent/system_prompt.rs`
**What changed:** Updated `default_system_prompt()` function to include comprehensive Nix usage instructions.

**Added section:** "Handling Missing Commands/Packages"
- Instructions to use `nix-shell -p` for quick commands
- Examples for Python, Node.js, C compilation
- Interactive development with `nix-shell` and `nix develop`
- Direct app execution with `nix run`
- **Critical rule:** NEVER use `nix profile install`

## Files Created

### 2. `.clankers/skills/nix-ephemeral-packages/SKILL.md`
A detailed skill document covering:
- Three main Nix approaches (nix-shell, nix run, nix develop)
- Decision tree for choosing the right tool
- Common patterns and examples
- Language-specific examples (Python, Node.js, Rust, C/C++)
- Package discovery methods

## How It Works

**Automatic Loading:**
1. The default system prompt is loaded for all agents unless overridden
2. Skills in `.clankers/skills/*/SKILL.md` are automatically discovered and available
3. Both mechanisms ensure agents know about Nix capabilities

**Priority:**
- System prompt: Always included (unless custom agent definition)
- Skills: Available as reference documentation

## Testing
✅ Verified syntax is valid
✅ Successfully ran Python script with: `nix-shell -p python3 --run "python3 hello_world.py"`
✅ Output: "Hello, World!"

## Result
Any agent (clankers or subagents) will now automatically:
1. Know to use Nix when commands aren't found
2. Have access to practical examples
3. Avoid problematic `nix profile` commands
4. Keep the system clean with ephemeral package usage
