#!/bin/sh

# License: CC0 1.0 Universal
# https://creativecommons.org/publicdomain/zero/1.0/legalcode

set -xe

. ./scripts/travis-doc-upload.cfg

giveup() {
    echo "not uploading docs: " $1
    exit
}

RUST_VERSION=`rustc --version`

case $RUST_VERSION in
     *beta*) giveup "$RUST_VERSION is not stable"
             ;;
     *nightly*) giveup "$RUST_VERSION is not stable"
                ;;
esac

[ "$TRAVIS_BRANCH" = master ] || giveup "Not on master branch"

[ "$TRAVIS_PULL_REQUEST" = false ] || giveup "This is pull request"

cargo doc

eval key=\$encrypted_${SSH_KEY_TRAVIS_ID}_key
eval iv=\$encrypted_${SSH_KEY_TRAVIS_ID}_iv

mkdir -p ~/.ssh
openssl aes-256-cbc -K $key -iv $iv -in scripts/quodlibetor-gh-deploy.enc -out ~/.ssh/id_rsa -d
chmod 600 ~/.ssh/id_rsa

rm -rf deploy_docs
git clone --branch gh-pages git@github.com:$REPO deploy_docs

cd deploy_docs
git config user.name "doc upload bot"
git config user.email "nobody@example.com"
rm -rf $DOC_ROOT
mv ../target/doc $DOC_ROOT
git add -A $DOC_ROOT
git commit -qm "doc upload for $PROJECT_NAME ($TRAVIS_REPO_SLUG)"
git push origin gh-pages

echo "successfully updated docs"
