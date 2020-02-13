using Fanhuaji_API.Class;
using Fanhuaji_API.Enum;
using System;
using System.Collections.Generic;
using System.ComponentModel;
using System.Linq;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Input;
namespace ConvertZZ
{
	/// <summary>
	/// Window_FanhuajiSetting.xaml 的互動邏輯
	/// </summary>
	public partial class Window_FanhuajiSetting : Window, INotifyPropertyChanged
	{
		private int replaceDicIndex = 0;

		public List<KeyValue> ReplaceList { get; set; } = new List<KeyValue>();

		public event PropertyChangedEventHandler PropertyChanged;

		public Window_FanhuajiSetting()
		{
			InitializeComponent();
			string[] names = Enum.GetNames(typeof(Enum_Modules));
			foreach (string s in names)
			{
				if ((from x in App.Settings.Fanhuaji_Setting.Modules
					 where x.ModuleName.ToString() == s
					 select x).Count() == 0)
				{
					App.Settings.Fanhuaji_Setting.Modules.Add(new Module((Enum_Modules)Enum.Parse(typeof(Enum_Modules), s), (bool?)null));
				}
			}
			base.DataContext = App.Settings.Fanhuaji_Setting;
			DataGrid_ReplaceList.DataContext = this;
		}

		private void DataGrid_MouseLeftButtonUp(object sender, MouseButtonEventArgs e)
		{
			if (e.OriginalSource is Border && !((sender as DataGrid).CurrentColumn.DependencyObjectType.Name != "DataGridCheckBoxColumn"))
			{
				Module module = ((Border)e.OriginalSource).BindingGroup.Items[0] as Module;
				if (!module.Enable.HasValue)
				{
					module.Enable = ((bool?)false);
				}
				else if (module.Enable == false)
				{
					module.Enable = ((bool?)true);
				}
				else
				{
					module.Enable = ((bool?)null);
				}
			}
		}

		private void RadioButton_Click(object sender, RoutedEventArgs e)
		{

			switch (replaceDicIndex)
			{
				case 0:
					App.Settings.Fanhuaji_Setting.UserPreReplace.Clear();
					App.Settings.Fanhuaji_Setting.UserPreReplace.AddRange(ReplaceList);
					break;
				case 1:
					App.Settings.Fanhuaji_Setting.UserPostReplace.Clear();
					App.Settings.Fanhuaji_Setting.UserPostReplace.AddRange(ReplaceList);
					break;
				case 2:
					App.Settings.Fanhuaji_Setting.UserProtectReplace.Clear();
					App.Settings.Fanhuaji_Setting.UserProtectReplace.AddRange(ReplaceList);
					break;
			}
			ReplaceList.Clear();
			string uid = (sender as RadioButton).Uid;
			switch (uid)
			{
				case "1":
					replaceDicIndex = 0;
					ReplaceList.AddRange(App.Settings.Fanhuaji_Setting.UserPreReplace);
					break;
				case "2":
					replaceDicIndex = 1;
					ReplaceList.AddRange(App.Settings.Fanhuaji_Setting.UserPostReplace);
					break;
				case "3":
					replaceDicIndex = 2;
					ReplaceList.AddRange(App.Settings.Fanhuaji_Setting.UserProtectReplace);
					break;
			}
			PropertyChanged?.Invoke(this, new PropertyChangedEventArgs("ReplaceList"));
		}

		private void Window_Closing(object sender, CancelEventArgs e)
		{
			App.Save();
		}

		private void DataGrid_ReplaceList_AutoGeneratingColumn(object sender, DataGridAutoGeneratingColumnEventArgs e)
		{
			if (e.PropertyName == "Key")
			{
				e.Column.Header = "搜尋";
			}
			else
			{
				e.Column.Header = "取代";
			}
		}
	}
}
