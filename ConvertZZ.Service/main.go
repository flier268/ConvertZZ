package main

import (
	"flag"
	"fmt"
	"net/http"
	"os"

	"github.com/gin-gonic/gin"
)

const version string = "1.0.0.0"

func usage() {
	fmt.Fprintf(os.Stderr, "Usage: ConvertZZ.Service [options]\n")
	flag.PrintDefaults()
}

func main() {
	var port int
	flag.IntVar(&port, "p", 8080, "Set http port")
	isPrintVersion := flag.Bool("v", false, "print version number")
	flag.Usage = usage
	flag.Parse()
	if *isPrintVersion {
		fmt.Println(version)
		os.Exit(0)
	}

	textConverterInit()
	r := gin.Default()
	r.GET("/version", getVersion)
	v1 := r.Group("/api/v1")
	{
		tc := v1.Group("/textConverter")
		{
			tc.GET("/", v1TextConverterHelp)
			tc.GET("/convert", v1TextConverterHelp)
			tc.GET("/seg", v1TextConverterHelp)
			tc.GET("/justConvert", v1TextConverterHelp)
			tc.GET("/convert/:option/*text", v1TextConverterConvert)
			tc.GET("/seg/:option/*text", v1TextConverterSeg)
			tc.GET("/justConvert/:option/*text", v1TextConverterJustConvert)
			tc.POST("/convert/:option", v1PostTextConverterConvert)
			tc.POST("/justConvert/:option", v1PostTextConverterJustConvert)
			tc.POST("/seg/:option", v1PostTextConverterSeg)
		}
	}
	r.RedirectFixedPath = true
	r.Run(fmt.Sprintf("127.0.0.1:%v", port))
}

func getVersion(context *gin.Context) {
	context.String(http.StatusOK, "%v", version)
}

func wrapResponse(context *gin.Context, output any, err error) {
	var r = struct {
		Output  any    `json:"output"`
		Status  string `json:"status"`
		Message string `json:"message"`
	}{
		Output:  output,
		Status:  "ok", // 預設狀態為ok
		Message: "",
	}
	if err != nil {
		r.Output = ""
		r.Status = "failed"     // 若出現任何err，狀態改為failed
		r.Message = err.Error() // Message回傳錯誤訊息
	}

	context.JSON(http.StatusOK, r)
}
