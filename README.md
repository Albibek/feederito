# Feederito is yet another Web-based UI Feed reader, suitable for AWS Free Tier

Feederito is minimalistic, self-hosted and aimed to be secure feed reader.
It is expected to be hosted on AWS, using Lambda and DynamoDB, which have good free tier limits and costs the author around 1,5$ per month.

The project is at early development stage yet. It is developed partially for personal use, partially to demonstrate a concept of highly-secure web application architecture.
The concept itself assumes zero-knowlege about the data on a backend. Although, for a feed reader zero-knowlege is an obvious overkill and is not implemented, the same concept can be used for more sensitive applications.

### Security concept
To get more protection from the XSS and other frontend attacks, the frontend logic is split into 2 parts: UI and worker.
When backend interaction is required, the UI sends data to worker, so the UI itself only responsible for drawing logic without knowing credentials or caring about sending data to the
backend.

Worker (based on web worker) stores all the credentials and uses them to sign the requests to lambda. The credentials on the worker side are encrypted with the master password and saved to local storage. With this neither the backend, nor the UI part know anything about the real data.

Worker is recommended to be deployed onto separate domain, so the LocalStorage is considered to be in other context by browser, therefore becoming not accessible by the UI.

Both, the UI and the worker are written in WebAssembly and Rust, which gives another protection from potential bugs and JS-specific errors.

### Single data model concept
Good old MVC approach plays new colors when applied to static typing on both frontend, backend and worker. Now the whole data model can be used as a library on any of these sides, and differences between them can be handled as derivatives(by having separate structures at each module) or even statically with cfg-pragmas.

### CSS theming concept
(see `styler` workspace) This is most probably now new. While ideas from Tachyons CSS are great, it is very annoying to put a big bunch of classes to multiple elements, like buttons only to make them look similar. So the idea is to declare a single class for button, but so that class is combined from other ready-to-use classes from tachyons. While SASS could be used for the same purpose, the styler project gives it another try and combines CSS by compiling new classes from the source.
Such approach also allows theming along with using any CSS framework at (actually low) price of having a full-blown CSS parser at build time.
