# Second Life Prim Messaging

## Server Setup

Set two different, randomly generated secrets in the server process environment:

```sh
export SL_MESSAGE_ADMIN_TOKEN="YOUR_LONG_RANDOM_ADMIN_TOKEN"
export SL_MESSAGE_DEVICE_TOKEN="YOUR_DIFFERENT_LONG_RANDOM_DEVICE_TOKEN"
```

Restart the rebuilt server and open `/slmessage` over HTTPS. Enter the admin token to load the object registry. Tokens are held only in page memory, not browser storage. The page and template script are public; all object data and changes require authentication. Do not put the admin token in a prim.

The registry is stored in Rust's `partitioned_array_rust::PartitionedArray`, with one record per prim and dynamically growing partitions. The complete array is saved atomically to `sl_message_data/objects.json`, relative to the server working directory. Override with `SL_MESSAGE_STORE`. Each successful poll or admin edit is persisted before returning success, including identities, per-prim secrets, desired text/color/opacity, revisions and acknowledgments, reported text, latest owner message, and timestamps. These records survive server restarts. Existing plain-map registries are automatically migrated at startup without changing prim identities or credentials. Preserve this file across deployments and restrict its permissions: it contains per-prim secrets. Missing tokens disable the corresponding API with HTTP 503; existing server routes still work. Missing storage starts empty; invalid storage fails startup instead of silently discarding data.

## Prim Setup

Use `second_life/sl_message.lsl` or download `/slmessage/script.lsl`. Set `SERVER` to your public HTTPS origin (without a trailing slash) and `DEVICE_TOKEN` to the server's device token. Place the script in every prim whose hover text you want to control. Use a valid publicly trusted TLS certificate reachable from Second Life.

Each prim generates an ID and secret with `llGenerateKey`, saved in linkset data under its actual prim UUID. No manual ID configuration is needed. Resets reuse the identity; rezzed copies with different prim UUIDs get distinct identities. Owner chat shows the generated ID (used as `:id` in the update route) and the actual prim UUID (sent as `object_key`); these are different identifiers. The secret is never printed. Each script controls only the prim containing it. Deleting its linkset data creates a new registry entry on the next reset. Unlinking can also change the available linkset data; keep IDs shown in owner chat for reference.

Prims register on their first poll. The dashboard refreshes every five seconds, and prims poll about every 15-18 seconds (with error backoff). The script honors the server's polling interval, bounded to 15-120 seconds plus jitter. Requests are never overlapped. Edits to text, color, and opacity target one ID. Empty text clears hover text. Text is limited to 254 UTF-8 bytes, not 254 Unicode characters. After applying an update, the script immediately polls again to acknowledge the revision; failed acknowledgments retry on subsequent polls. Newer edits replace older pending edits; this is a latest-state service, not a chat queue. Offline edits persist until a prim returns. Online means a poll within the last 60 seconds.

## From Second Life To The Server

The prim reports its name, region, actual hover text, and applied revision every poll. Owner chat on channel `-934833` becomes its latest object message, for example:

```text
/-934833 Hello from Second Life
```

All nearby prim scripts owned by that avatar can receive that message. It does not change hover text locally. Local hover text changes made by another script appear as reported text; they do not overwrite the server's desired text. Only a new server revision or script reset reapplies desired text. The latest owner message is retained, not a history; the script's local message resets when the script resets.

Use `/-934833 id` to print nearby prim IDs without sending a message. To send a message from only one prim, use `/-934833 @<prim-id> Hello from this prim`, replacing `<prim-id>` with its generated ID. Other prims ignore messages targeted to that ID. Untargeted messages retain the broadcast behavior above.

## API

All bodies are JSON, at most 4096 bytes. Responses must not be cached.

- `GET /slmessage` or `/slmessage/`: control page.
- `GET /slmessage/script.lsl`: template script, without credentials.
- `GET /slmessage/objects`: registry, using `Authorization: Bearer <admin token>`; never returns per-prim secrets.
- `POST /slmessage/objects/:id`: admin update, body `{"text":"Hello","color":[1,1,1],"alpha":1}`.
- `POST /slmessage/poll`: register/report/fetch, using the device bearer token. Body contains `id`, `secret`, `object_key`, `name`, `region`, `applied_revision`, `reported_text`, `color`, `alpha`, and `message`. Initial revision is `-1`. Reply contains `revision`, `text`, `color`, `alpha`, and `poll_seconds`. Existing IDs require their original prim key and secret as well as the shared device token.

The registry allows up to 500 prims. Treat scripts containing the shared device token as trusted: holders can enroll new prims. Rotate that token on the server and in every script if exposed. This does not provision independent device enrollment permissions or provide a multi-server shared database. Back up storage before manual maintenance.