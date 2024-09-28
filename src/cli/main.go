package main

import (
	"os"

	"github.com/spf13/cobra"
)

func main() {
	var rootCmd = &cobra.Command{
		Use:     "yarrow",
		Short:   "",
		Long:    "",
		Version: "",
		Run: func(cmd *cobra.Command, args []string) {
			if len(args) <= 0 {
				cmd.Help()
				os.Exit(0)
			}
		},
	}

	var runCmd = &cobra.Command{
		Use:   "run",
		Short: "",
		Long:  "",
		Run: func(cmd *cobra.Command, args []string) {
		},
	}

	var buildCmd = &cobra.Command{
		Use:   "build",
		Short: "",
		Long:  "",
		Run: func(cmd *cobra.Command, args []string) {
            // quiet, _ := cmd.Flags().GetBool("quiet")
            // verbose, _ := cmd.Flags().GetBool("verbose")
            // optimization, _ := cmd.Flags().GetInt("optimization")
		},
	}
    buildCmd.Flags().IntP("optimization", "O", 0, "")
    buildCmd.Flags().BoolP("quiet", "q", false, "")
    buildCmd.Flags().BoolP("verbose", "v", false, "")

	rootCmd.AddCommand(runCmd, buildCmd)

	if error := rootCmd.Execute(); error != nil {
		os.Exit(1)
	}

	os.Exit(0)
}
