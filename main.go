package main

import (
	"html/template"
	"log"
	"net/http"
)

var (
	ts = template.Must(template.ParseFiles(
		"templates/login.html",
		"templates/snippet-form.html",
	))
	story *Story
)

func useTemplate(w http.ResponseWriter, templateName string, data interface{}) {
	err := ts.ExecuteTemplate(w, templateName, data)
	if err != nil {
		log.Printf("Unable to generate template: %v", err)
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}
}

func welcomeHandler(w http.ResponseWriter, r *http.Request) {
	useTemplate(w, "login.html", nil)
}

func snippetFormHandler(w http.ResponseWriter, r *http.Request) {
	useTemplate(w, "snippet-form.html", nil)
}

func snippetSubmitHandler(w http.ResponseWriter, r *http.Request) {
	http.Redirect(w, r, "/", 303)
}

func main() {
	http.HandleFunc("/", welcomeHandler)
	http.ListenAndServe(":9000", nil)
}
