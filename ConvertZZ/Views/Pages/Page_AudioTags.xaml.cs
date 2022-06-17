using System;
using System.Collections.Generic;
using System.Collections.ObjectModel;
using System.ComponentModel;
using System.Diagnostics;
using System.IO;
using System.Linq;
using System.Text;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Input;
using ConvertZZ.Core.Helpers;
using ConvertZZ.Moudle;
using ConvertZZ.ViewModels;
using Microsoft.Win32;
using TagLib;
using static Fanhuaji_API.Fanhuaji;
using File = System.IO.File;

namespace ConvertZZ.Views.Pages
{
    /// <summary>
    /// Page_AudioTags.xaml 的互動邏輯
    /// </summary>
    public partial class Page_AudioTags : Page
    {
        private AudioTagsPageVIewModel VIewModel { get; }

        public Page_AudioTags(EAudioFormat format)
        {
            InitializeComponent();
            VIewModel = (AudioTagsPageVIewModel)DataContext;

            VIewModel.FilterList = new(App.Settings.FileConvert.GetFilterList());
            VIewModel.SelectedFilter = VIewModel.FilterList.FirstOrDefault();
        }

        public Page_AudioTags(EAudioFormat format, string[] FileNames) : this(format)
        {
            if (FileNames == null)
                return;
            VIewModel.ImportFileNames(FileNames);
        }

        private void Page_Loaded(object sender, RoutedEventArgs e)
        {
        }
    }
}