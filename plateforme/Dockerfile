# The image of the `plateforme` service, used by the `deploiement` profile of
# `docker-compose.plateforme.yml`.
#
# ⚠️ THIS FILE IS NOT PRESCRIBED BY THE P5 PLAN, and that is declared rather
# than kept quiet: the plan's "File structure" lists
# `deploiement/nginx.conf` and `deploiement/README.md`, but no image for the
# `plateforme` service that it nevertheless asks task 13 to declare. Without
# it, the `deploiement` profile describes itself but does not start. It is a gap
# in the plan, filled as minimally as possible.
#
# 🔴 THE MAJOR VERSION FOLLOWS `package.json`, which declares
# `"node": ">=24.0.0 <25.0.0"`. An image of another major might
# start, and would diverge from the engine on which the 423 tests are green.
# 🔴 THE BUILD CONTEXT IS THE ROOT OF THE REPOSITORY, AND NOT `plateforme/`.
# IT IS NOT A CONVENIENCE, IT IS A MEASURED NECESSITY.
#
# Three PRODUCTION files import `../../../proto/ts/plateforme` —
# `agents/canal.ts`, `agents/registre.ts`, `apps/catalogue.ts` —, that is
# a path that LEAVES `plateforme/`. With a context bounded to `plateforme/`,
# that directory cannot enter the image, and the service DIES at
# load time on `ERR_MODULE_NOT_FOUND … file:///opt/proto/ts/plateforme`.
#
# ⚠️ NOTHING SAW IT BEFORE CRITERION ⑤ OF THE ACCEPTANCE: the 433 tests
# run from the tree, where the relative path exists; `docker build`
# SUCCEEDS, since nothing is resolved at build time; `docker compose
# config` and `nginx -t` have no opinion. **The image built without
# error and did not start.** Found on 20 August 2026 by launching the
# `deploiement` profile for the first time.
#
# The trade-off is named: a wide context would send the whole repository to the
# daemon. `plateforme/Dockerfile.dockerignore` bounds it — BuildKit reads the
# `.dockerignore` NAMED AFTER THE DOCKERFILE when it exists, which avoids
# placing a `.dockerignore` at the root, which would ALSO apply to the image of the
# historical Guacamole (`./Dockerfile`) and would change a build that does not
# belong to us.
FROM node:24-alpine

WORKDIR /opt/plateforme

# The manifests first, and the installation afterwards: the installation layer
# is only redone if the dependencies change, not at every source edit.
COPY plateforme/package.json plateforme/package-lock.json ./

# 🔴 `npm ci` AND NOT `npm install`: it requires `package-lock.json` to be
# consistent with `package.json` and refuses to REWRITE it. An image built with
# `install` could ship versions that nobody has tested.
#
# ⚠️ THE DEVELOPMENT DEPENDENCIES ARE INSTALLED, and it is not an
# oversight: `npm start` runs `tsx src/index.ts`, and `tsx` is a
# `devDependency`. An `--omit=dev` would produce an image that fails at startup
# with "tsx: not found". The service therefore runs from the TypeScript
# SOURCES, without a compilation step — the same path as
# `npm run start` in development, which removes any divergence between what
# is proven and what is deployed.
RUN npm ci

COPY plateforme/tsconfig.json ./
COPY plateforme/src ./src

# 🔴 `proto/ts` IS PLACED AT `/opt/proto/ts`, AND THE ADDRESS IS NOT FREE: the
# sources live at `/opt/plateforme/src/<folder>/<file>.ts` and climb up
# three levels (`../../../proto/ts/plateforme`), which designates exactly
# `/opt/proto/ts/plateforme`. It is the URL Node named in the error.
# Moving `WORKDIR` without moving this would reopen the same defect.
#
# ⚠️ ONLY `proto/ts` ENTERS, not `proto/src` (the Rust crate): the service has
# no need for it, and an image that ships what it does not run merely
# widens its surface.
COPY proto/ts /opt/proto/ts

# ⚠️ NO `EXPOSE`, AND NO PORT PUBLISHED in the compose file. The
# service listens on `PLATEFORME_HOTE`, whose value is the SERVICE NAME: Node
# resolves that name and binds to the container address on the internal network. Only
# the proxy reaches it, and that is what spec §9 requires — the platform never
# terminates TLS.
#
# ⚠️ `PLATEFORME_HOTE` has NO default: if the environment does not set it,
# `lireConfig` THROWS at startup rather than listening everywhere.
CMD ["npm", "start"]
