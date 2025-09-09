I need to recursively sync a folder with  tens of thousands of markdown files and linked images between multiple macs and android. I use syncthing but it's unreliable, slow, on Android it drains battery super fast. Also, syncthing does not handle merge problems. I don't want to use the paid obsidian sync, but create a super efficient rust implementation that works cross platform. Ask clarification questions first

I'd like to understand your requirements better before we dive into the Rust implementation. Here are some key questions:

Sync Requirements:

1. Q: Do you need real-time sync or would periodic sync (every few minutes/hours) be sufficient?
A: Real-time or near real time (sub minute to sync between two devices)
2. Q: How do you want to handle conflicts when the same file is modified on multiple devices simultaneously?
A: Do sensible merging if extra content, use latest for deletions. Use something predictable as users cannot work on different systems at the same time. the sync is for single-player not team sync.
3. Q: Should it work offline and sync when connectivity is restored?
A: Sure.
File Structure & Linking:
4. Q: Are the images referenced in markdown using relative paths, absolute paths, or a mix?
A: Relative within the root folder that is synced, most often you will have `/images` (or similar name) and all markdowns will reference via `![filenmae.jpg|png]()` regardless where in folder they are. 
5. Q: Do you need to preserve file timestamps and metadata? A: Timestamps for last changed and created, yes, metadata must be in file (via yaml frontmatter)
6. Q: Are there any specific markdown extensions or formats you use that might affect linking? A: No, regula markdown

Platform Specifics:
7. Q: For Android: Would a background service be acceptable, or do you prefer manual/triggered sync? A: Background service is a must, manual sync error-prone and no nice UX
8. Q: Do you need a GUI application or would command-line tools suffice? Any preference for sync protocol (HTTP/HTTPS, custom TCP, etc.)? A: CLI enough, you decide the protocol

Infrastructure:
9. Q: Do you have a server/NAS you could use as a central sync hub, or does it need to be peer-to-peer? Any bandwidth/data usage constraints, especially on mobile? I have a VPS (linux), but it should also be peer-to-peer between two computers or computer <> android phone, and of course then also between multiple peers

Conflict Resolution:
10. Q: For merge conflicts: would you prefer automatic resolution strategies (last-write-wins, content merging) or manual resolution prompts?
    Should there be version history/backup of conflicted files? - A: see question 2, find the best treatment for single-user with many machines use case. they will not work at the same time at two machines, but sync must be (near) instant and merging must make sense, no manual resolution will be annoying.