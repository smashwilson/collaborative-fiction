package main

import (
	"io/ioutil"
	"log"
	"net/http"
	"os"

	"github.com/gorilla/securecookie"
	"github.com/stretchr/gomniauth"
	"github.com/stretchr/gomniauth/providers/github"
	"github.com/stretchr/gomniauth/providers/google"
	"github.com/stretchr/goweb"
	"github.com/stretchr/goweb/context"
)

var cookieGen *securecookie.SecureCookie

func securityKey(filename string, length int) ([]byte, error) {
	file, err := os.Open(filename)
	switch {
	case err != nil:
		return ioutil.ReadAll(file)
	case os.IsNotExist(err):
		secret := securecookie.GenerateRandomKey(length)
		err := ioutil.WriteFile(filename, secret, 0600)
		if err != nil {
			return nil, err
		}
		return secret, nil
	default:
		return nil, err
	}
}

func registerAuthRoutes() error {
	providerSecret, err := securityKey(".provider.secret", 64)
	if err != nil {
		return err
	}

	cookieHash, err := securityKey(".cookiehash.secret", 64)
	if err != nil {
		return err
	}

	cookieBlock, err := securityKey(".cookieblock.secret", 32)
	if err != nil {
		return err
	}

	gomniauth.SetSecurityKey(string(providerSecret))

	cookieGen = securecookie.New(cookieHash, cookieBlock)

	gomniauth.WithProviders(
		google.New(config.GoogleKey, config.GoogleSecret, absURL("auth/google/callback")),
		github.New(config.GitHubKey, config.GitHubSecret, absURL("auth/github/callback")),
	)

	goweb.Map("GET", path("auth/{provider}/login"), authLoginHandler)
	goweb.Map("GET", path("auth/{provider}/callback"), authCallbackHandler)

	return nil
}

func authLoginHandler(ctx context.Context) error {
	providerName := ctx.PathValue("provider")

	provider, err := gomniauth.Provider(providerName)
	if err != nil {
		log.Printf("Unable to locate requested provider [%s]: %v", providerName, err)
		return goweb.Respond.WithStatus(ctx, http.StatusNotFound)
	}

	authURL, err := provider.GetBeginAuthURL(nil, nil)
	if err != nil {
		log.Printf("Unable to generate auth URL for provider [%s]: %v", providerName, err)
		return goweb.Respond.WithStatus(ctx, http.StatusInternalServerError)
	}

	return goweb.Respond.WithRedirect(ctx, authURL)
}

func authCallbackHandler(ctx context.Context) error {
	providerName := ctx.PathValue("provider")

	provider, err := gomniauth.Provider(providerName)
	if err != nil {
		log.Printf("Unable to locate requested provider [%s]: %v", providerName, err)
		return goweb.Respond.WithStatus(ctx, http.StatusNotFound)
	}

	creds, err := provider.CompleteAuth(ctx.QueryParams())
	if err != nil {
		log.Printf("Unable to compute authentication against provider [%s]: %v", providerName, err)
		return goweb.Respond.WithStatus(ctx, http.StatusInternalServerError)
	}

	user, err := provider.GetUser(creds)
	if err != nil {
		log.Printf("Unable to retrieve user from provider [%s] results: %v", providerName, err)
		return goweb.Respond.WithStatus(ctx, http.StatusInternalServerError)
	}

	cookieData := map[string]string{
		"name":   user.Name(),
		"email":  user.Email(),
		"avatar": user.AvatarURL(),
	}
	encoded, err := cookieGen.Encode("user", cookieData)
	if err != nil {
		log.Printf("Unable to generate cookie: %v", err)
		return goweb.Respond.WithStatus(ctx, http.StatusInternalServerError)
	}

	http.SetCookie(ctx.HttpResponseWriter(), &http.Cookie{
		Name:  "user",
		Value: encoded,
		Path:  config.Root,
	})

	return goweb.Respond.WithRedirect(ctx, config.Root)
}
